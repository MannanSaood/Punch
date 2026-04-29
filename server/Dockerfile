FROM golang:1.22-alpine AS builder

WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download

COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -o punch-server ./cmd/main.go

FROM alpine:3.19
RUN apk --no-cache add ca-certificates

WORKDIR /app
COPY --from=builder /app/punch-server .

EXPOSE 8080

ENV PUNCH_PORT=8080
ENV PUNCH_RELAY_ENABLED=true
ENV PUNCH_MAX_SESSIONS=1000

CMD ["./punch-server"]
