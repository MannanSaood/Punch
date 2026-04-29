package config

import (
	"os"
	"strconv"
)

// Config holds all server configuration loaded from environment variables.
type Config struct {
	Port         string
	RelayEnabled bool
	MaxSessions  int
}

// Load reads configuration from environment variables with sensible defaults.
func Load() *Config {
	return &Config{
		Port:         getEnv("PUNCH_PORT", "8080"),
		RelayEnabled: getEnvBool("PUNCH_RELAY_ENABLED", true),
		MaxSessions:  getEnvInt("PUNCH_MAX_SESSIONS", 1000),
	}
}

func getEnv(key, fallback string) string {
	if val := os.Getenv(key); val != "" {
		return val
	}
	return fallback
}

func getEnvBool(key string, fallback bool) bool {
	val := os.Getenv(key)
	if val == "" {
		return fallback
	}
	b, err := strconv.ParseBool(val)
	if err != nil {
		return fallback
	}
	return b
}

func getEnvInt(key string, fallback int) int {
	val := os.Getenv(key)
	if val == "" {
		return fallback
	}
	i, err := strconv.Atoi(val)
	if err != nil {
		return fallback
	}
	return i
}
