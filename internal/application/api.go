package application

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os/exec"
	"runtime"
	"time"

	"lwsc-bot/internal/infrastructure"
)

type API struct {
	config *infrastructure.Config
}

func StartAPIServer(cfg *infrastructure.Config, port int) {
	api := &API{config: cfg}

	http.HandleFunc("/api/config", api.handleConfig)
	http.HandleFunc("/", api.handleStatic)

	addr := fmt.Sprintf(":%d", port)
	fmt.Printf("Web UI server running at http://localhost%s\n", addr)
	
	go func() {
		if err := http.ListenAndServe(addr, nil); err != nil {
			fmt.Printf("HTTP server failed: %v\n", err)
		}
	}()
}

func (a *API) handleConfig(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Access-Control-Allow-Origin", "*")
	
	if r.Method == http.MethodGet {
		json.NewEncoder(w).Encode(a.config)
		return
	}
	
	if r.Method == http.MethodPost {
		// In a full implementation, we would decode the updated config from JSON
		// and use yaml.Marshal to save back to config/*.yaml
		// For the scope of this prototype, we'll acknowledge the save.
		var newCfg infrastructure.Config
		if err := json.NewDecoder(r.Body).Decode(&newCfg); err != nil {
			http.Error(w, err.Error(), http.StatusBadRequest)
			return
		}
		
		fmt.Println("[API] Configuration update received via Web UI")
		json.NewEncoder(w).Encode(map[string]string{"status": "success"})
		return
	}
	
	http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
}

func (a *API) handleStatic(w http.ResponseWriter, r *http.Request) {
	// Serve the ui/index.html file
	http.ServeFile(w, r, "ui/index.html")
}

// OpenBrowser opens the specified URL in the default browser of the user.
func OpenBrowser(url string) {
	var err error

	switch runtime.GOOS {
	case "linux":
		err = exec.Command("xdg-open", url).Start()
	case "windows":
		err = exec.Command("rundll32", "url.dll,FileProtocolHandler", url).Start()
	case "darwin":
		err = exec.Command("open", url).Start()
	default:
		err = fmt.Errorf("unsupported platform")
	}

	if err != nil {
		fmt.Printf("Failed to open browser: %v\n", err)
	} else {
		// Add a slight delay to ensure the browser has time to open the page
		time.Sleep(500 * time.Millisecond)
		fmt.Println("Configuration window opened in browser.")
	}
}
