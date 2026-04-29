package main

import (
	"fmt"
	"log"
	"net/http"
	"os"

	"github.com/MannanSaood/punch/server/internal/config"
	"github.com/MannanSaood/punch/server/internal/signaling"
)

func main() {
	cfg := config.Load()

	hub := signaling.NewHub()
	go hub.Run()

	http.HandleFunc("/ws", hub.HandleWebSocket)
	http.HandleFunc("/health", handleHealth)

	addr := fmt.Sprintf(":%s", cfg.Port)
	log.Printf("Punch signalling server running on %s", addr)
	log.Printf("Relay enabled: %v", cfg.RelayEnabled)
	log.Printf("Max sessions: %d", cfg.MaxSessions)

	if err := http.ListenAndServe(addr, nil); err != nil {
		log.Fatalf("Server failed: %v", err)
		os.Exit(1)
	}
}

func handleHealth(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	fmt.Fprintln(w, `{"status":"ok","service":"punch-signalling"}`)
}
