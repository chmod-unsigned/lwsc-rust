package infrastructure

import (
	"fmt"
	"os"

	"gopkg.in/yaml.v3"

	"lwsc-bot/internal/domain"
)

type Config struct {
	Actions   map[string]domain.Action
	States    map[string]domain.State
	Buttons   map[string]domain.Button
	Shortcuts map[string]string // e.g. "toggle_pause": "ctrl+p"
	Sequences map[string]domain.Sequence
}

// LoadConfig loads the simplified configuration from YAML files.
// We ignore sequences and priority per user requirements.
func LoadConfig(configDir string) (*Config, error) {
	cfg := &Config{
		Actions:   make(map[string]domain.Action),
		States:    make(map[string]domain.State),
		Buttons:   make(map[string]domain.Button),
		Shortcuts: make(map[string]string),
	}

	// Load Actions
	if err := loadYAML(configDir+"/actions.yaml", &cfg.Actions); err != nil {
		return nil, fmt.Errorf("failed to load actions: %w", err)
	}

	// Load States
	if err := loadYAML(configDir+"/states.yaml", &cfg.States); err != nil {
		return nil, fmt.Errorf("failed to load states: %w", err)
	}

	// Load Buttons
	if err := loadYAML(configDir+"/buttons.yaml", &cfg.Buttons); err != nil {
		return nil, fmt.Errorf("failed to load buttons: %w", err)
	}

	// Load Shortcuts
	if err := loadYAML(configDir+"/shortcuts.yaml", &cfg.Shortcuts); err != nil {
		return nil, fmt.Errorf("failed to load shortcuts: %w", err)
	}

	// Load Sequences
	if err := loadYAML(configDir+"/sequences.yaml", &cfg.Sequences); err != nil {
		// Non-fatal if sequences.yaml doesn't exist, but we will print a warning
		fmt.Printf("Warning: failed to load sequences: %v\n", err)
	}

	// Post-process to inject names (map keys) into the structs
	for k, v := range cfg.Actions {
		v.Name = k
		cfg.Actions[k] = v
	}
	for k, v := range cfg.States {
		v.Name = k
		cfg.States[k] = v
	}
	for k, v := range cfg.Buttons {
		v.Name = k
		cfg.Buttons[k] = v
	}
	for k, v := range cfg.Sequences {
		v.Name = k
		cfg.Sequences[k] = v
	}

	return cfg, nil
}

func loadYAML(filename string, out interface{}) error {
	data, err := os.ReadFile(filename)
	if err != nil {
		return err
	}
	return yaml.Unmarshal(data, out)
}
