package signaling

import (
	"encoding/json"
	"log"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/websocket"
)

const (
	handshakeTimeout = 5 * time.Minute
	writeTimeout     = 10 * time.Second
	readTimeout      = 5 * time.Minute
)

// MessageType defines the type of signalling message.
type MessageType string

const (
	MsgRegister  MessageType = "register"   // peer registers with a token code
	MsgWaiting   MessageType = "waiting"    // server tells peer to wait
	MsgMatched   MessageType = "matched"    // server tells peers they found each other
	MsgEndpoint  MessageType = "endpoint"   // peer shares its STUN-derived public endpoint
	MsgRelay     MessageType = "relay"      // fallback: relay a packet to the other peer
	MsgHandshake  MessageType = "handshake"
	MsgPublicKey  MessageType = "publickey"  // direct connection established, server exits
	MsgError     MessageType = "error"      // something went wrong
)

// Message is the envelope for all signalling communication.
type Message struct {
	Type    MessageType     `json:"type"`
	Code    string          `json:"code,omitempty"`
	Payload json.RawMessage `json:"payload,omitempty"`
}

// Endpoint holds a peer's STUN-derived public address.
type Endpoint struct {
	IP   string `json:"ip"`
	Port int    `json:"port"`
}

// Session represents two peers trying to connect.
type Session struct {
	Code      string
	PeerA     *Peer
	PeerB     *Peer
	CreatedAt time.Time
}

// Peer represents a connected WebSocket client.
type Peer struct {
	conn *websocket.Conn
	send chan []byte
}

// Hub manages all active sessions.
// It is the only stateful component in the server.
// All state is in-memory and discarded after handshake.
type Hub struct {
	mu       sync.RWMutex
	sessions map[string]*Session // code -> session
}

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool {
		// Allow all origins for self-hosted deployments.
		// In production, restrict this to your domain.
		return true
	},
}

// NewHub creates a new Hub.
func NewHub() *Hub {
	return &Hub{
		sessions: make(map[string]*Session),
	}
}

// Run starts background cleanup of timed-out sessions.
func (h *Hub) Run() {
	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()

	for range ticker.C {
		h.cleanupExpiredSessions()
	}
}

// HandleWebSocket upgrades the connection and starts the peer lifecycle.
func (h *Hub) HandleWebSocket(w http.ResponseWriter, r *http.Request) {
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("WebSocket upgrade failed: %v", err)
		return
	}

	peer := &Peer{
		conn: conn,
		send: make(chan []byte, 64),
	}

	go peer.writePump()
	peer.readPump(h)
}

// readPump handles incoming messages from a peer.
func (p *Peer) readPump(h *Hub) {
	defer p.conn.Close()

	for {
		p.conn.SetReadDeadline(time.Now().Add(readTimeout))
		_, raw, err := p.conn.ReadMessage()
		if err != nil {
			log.Printf("Read error: %v", err)
			h.removePeer(p)
			return
		}

		var msg Message
		if err := json.Unmarshal(raw, &msg); err != nil {
			p.sendError("invalid message format")
			continue
		}

		h.handleMessage(p, msg)
	}
}

// writePump sends queued messages to the WebSocket connection.
func (p *Peer) writePump() {
	defer p.conn.Close()

	for data := range p.send {
		p.conn.SetWriteDeadline(time.Now().Add(writeTimeout))
		if err := p.conn.WriteMessage(websocket.TextMessage, data); err != nil {
			log.Printf("Write error: %v", err)
			return
		}
	}
}

// handleMessage routes incoming messages to the correct handler.
func (h *Hub) handleMessage(p *Peer, msg Message) {
	switch msg.Type {

	case MsgRegister:
		// Peer is registering with a token code.
		// Either creates a new session (first peer) or completes one (second peer).
		h.handleRegister(p, msg.Code)

	case MsgPublicKey:
		h.handleEndpoint(p, msg)

	case MsgEndpoint:
		// Peer is sharing its STUN endpoint.
		// Forward it to the other peer in the session.
		h.handleEndpoint(p, msg)

	case MsgHandshake:
		// Direct connection established between peers.
		// Server's job is done — destroy the session.
		h.handleHandshakeComplete(p)

	case MsgRelay:
		// Direct connection failed — forward encrypted bytes to other peer.
		h.handleRelay(p, msg)

	default:
		p.sendError("unknown message type")
	}
}

// handleRegister processes a peer registering with a token code.
func (h *Hub) handleRegister(p *Peer, code string) {
	if code == "" {
		p.sendError("code is required")
		return
	}

	h.mu.Lock()
	defer h.mu.Unlock()

	session, exists := h.sessions[code]

	if !exists {
		// First peer — create session and wait.
		h.sessions[code] = &Session{
			Code:      code,
			PeerA:     p,
			CreatedAt: time.Now(),
		}
		p.sendMsg(Message{Type: MsgWaiting, Code: code})
		log.Printf("Session created: %s", code)
		return
	}

	if session.PeerB != nil {
		// Session already has two peers — reject.
		p.sendError("session full")
		return
	}

	// Second peer — complete the session and notify both.
	session.PeerB = p

	matched, _ := json.Marshal(map[string]string{"code": code, "role": "initiator"})
	session.PeerA.sendMsg(Message{Type: MsgMatched, Code: code, Payload: matched})

	matched2, _ := json.Marshal(map[string]string{"code": code, "role": "receiver"})
	session.PeerB.sendMsg(Message{Type: MsgMatched, Code: code, Payload: matched2})

	log.Printf("Session matched: %s — both peers connected", code)
}

// handleEndpoint forwards a peer's STUN endpoint to its matched peer.
func (h *Hub) handleEndpoint(sender *Peer, msg Message) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	other := h.getOtherPeer(sender, msg.Code)
	if other == nil {
		sender.sendError("peer not found")
		return
	}

	// Forward the endpoint to the other peer.
	// This is the only thing the server learns about the peers:
	// their public IP/port, temporarily, for hole punching only.
	other.sendMsg(msg)
}

// handleHandshakeComplete destroys the session — server's job is done.
func (h *Hub) handleHandshakeComplete(p *Peer) {
	h.mu.Lock()
	defer h.mu.Unlock()

	for code, session := range h.sessions {
		if session.PeerA == p || session.PeerB == p {
			delete(h.sessions, code)
			log.Printf("Session %s complete — server exiting session", code)
			return
		}
	}
}

// handleRelay forwards encrypted bytes to the other peer.
// The server cannot read the payload — it's end-to-end encrypted by the clients.
func (h *Hub) handleRelay(sender *Peer, msg Message) {
	h.mu.RLock()
	defer h.mu.RUnlock()

	other := h.getOtherPeer(sender, msg.Code)
	if other == nil {
		return
	}

	other.sendMsg(msg)
}

// removePeer cleans up any session involving this peer.
func (h *Hub) removePeer(p *Peer) {
	h.mu.Lock()
	defer h.mu.Unlock()

	for code, session := range h.sessions {
		if session.PeerA == p || session.PeerB == p {
			// Notify the other peer if they're still connected.
			other := session.PeerA
			if other == p {
				other = session.PeerB
			}
			if other != nil {
				other.sendError("peer disconnected")
			}
			delete(h.sessions, code)
			log.Printf("Session %s removed — peer disconnected", code)
			return
		}
	}
}

// getOtherPeer returns the other peer in the same session.
func (h *Hub) getOtherPeer(p *Peer, code string) *Peer {
	session, exists := h.sessions[code]
	if !exists {
		return nil
	}
	if session.PeerA == p {
		return session.PeerB
	}
	if session.PeerB == p {
		return session.PeerA
	}
	return nil
}

// cleanupExpiredSessions removes sessions that exceeded the handshake timeout.
func (h *Hub) cleanupExpiredSessions() {
	h.mu.Lock()
	defer h.mu.Unlock()

	now := time.Now()
	for code, session := range h.sessions {
		if session.PeerB != nil && now.Sub(session.CreatedAt) > handshakeTimeout || session.PeerB == nil && now.Sub(session.CreatedAt) > 2*time.Minute {
			log.Printf("Session %s expired — cleaning up", code)
			if session.PeerA != nil {
				session.PeerA.sendError("session timeout")
			}
			if session.PeerB != nil {
				session.PeerB.sendError("session timeout")
			}
			delete(h.sessions, code)
		}
	}
}

// sendMsg sends a structured message to the peer.
func (p *Peer) sendMsg(msg Message) {
	data, err := json.Marshal(msg)
	if err != nil {
		log.Printf("Failed to marshal message: %v", err)
		return
	}
	select {
	case p.send <- data:
	default:
		log.Printf("Peer send buffer full — dropping message")
	}
}

// sendError sends an error message to the peer.
func (p *Peer) sendError(reason string) {
	payload, _ := json.Marshal(map[string]string{"reason": reason})
	p.sendMsg(Message{Type: MsgError, Payload: payload})
}