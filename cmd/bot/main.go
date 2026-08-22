package main

import (
	"io"
	"log"
	"os"
	"os/signal"
	"syscall"
	"time"

	"lwsc-bot/internal/application"
	"lwsc-bot/internal/infrastructure"
)

func main() {
	// Setup logging to file and stdout
	f, err := os.OpenFile("bot.log", os.O_RDWR|os.O_CREATE|os.O_APPEND, 0666)
	if err != nil {
		log.Fatalf("error opening log file: %v", err)
	}
	defer f.Close()
	log.SetOutput(io.MultiWriter(os.Stdout, f))
	log.SetFlags(0)

	log.Println("Starting Last War Survival Bot (Go Redesign)...")

	// Load configuration
	cfg, err := infrastructure.LoadConfig("config")
	if err != nil {
		log.Fatalf("Failed to load config: %v", err)
	}

	log.Printf("Loaded %d actions, %d states, %d buttons, %d shortcuts\n",
		len(cfg.Actions), len(cfg.States), len(cfg.Buttons), len(cfg.Shortcuts))

	// Setup Infrastructure
	visionSvc, err := infrastructure.NewVisionService("config", cfg)
	if err != nil {
		log.Fatalf("Failed to initialize vision service: %v", err)
	}
	inputSim := infrastructure.NewInputSimulator()

	// Setup Engine
	engine := application.NewEngine(cfg, visionSvc, inputSim)

	// Setup API Server (Web UI)
	application.StartAPIServer(cfg, 8080)

	// Register shortcuts
	if pauseKey, ok := cfg.Shortcuts["toggle_pause"]; ok {
		inputSim.RegisterShortcut(pauseKey, engine.TogglePause)
	}

	if configKey, ok := cfg.Shortcuts["open_config"]; ok {
		inputSim.RegisterShortcut(configKey, func() {
			application.OpenBrowser("http://localhost:8080")
		})
	}

	// Register Action shortcuts (e.g. action "alliance_help" mapped to "ctrl+1")
	for name, action := range cfg.Actions {
		if action.Shortcut != "" {
			actionRef := action // Capture variable for closure
			actionName := name
			inputSim.RegisterShortcut(action.Shortcut, func() {
				// Don't trigger shortcuts if the game doesn't have focus
				if !application.IsGameFocused("Last War") {
					return
				}
				
				log.Printf("\n[%s] [SHORTCUT] Manual override triggered: %s\n", time.Now().Format("15:04:05.000"), actionName)
				// Use fallback 1920x1080 resolution for manual trigger if screen dimension is unknown
				wx, wy, ww, wh, err := engine.GetGameWindowBounds("Last War")
				if err == nil {
					engine.ExecuteSingleAction(actionRef, float64(ww), float64(wh), wx, wy)
				} else {
					engine.ExecuteSingleAction(actionRef, 1920.0, 1080.0, 0, 0)
				}
			})
		}
	}

	// Panic / Kill switch
	inputSim.RegisterShortcut("ctrl+k", func() {
		log.Println("\n[URGENCE] Bot stoppé immédiatement par ctrl+k !")
		os.Exit(0)
	})

	go engine.Run()

	// Wait for interrupt signal to gracefully shutdown the bot
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)
	<-sigChan

	log.Println("\nShutting down bot...")
}
