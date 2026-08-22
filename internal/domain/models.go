package domain

// ROI represents a Region of Interest as normalized coordinates (0.0 to 1.0).
type ROI struct {
	XMin float64 `yaml:"xmin"`
	XMax float64 `yaml:"xmax"`
	YMin float64 `yaml:"ymin"`
	YMax float64 `yaml:"ymax"`
}

// State represents a recognized screen state.
type State struct {
	Name          string
	DisplayName   string   `yaml:"display_name"`
	Type          string   `yaml:"type"`
	Templates     string   `yaml:"templates"`      // Can be a file or a folder
	ROI           *ROI     `yaml:"roi"`
	MinConfidence float64  `yaml:"min_confidence"`
	ParentStates  []string `yaml:"parent_states"` // Adapted from single/list 'parent'
	Description   string   `yaml:"description"`
}

// Button represents a clickable element.
type Button struct {
	Name          string
	DisplayName   string   `yaml:"display_name"`
	ParentStates  []string `yaml:"parent_states"`
	TargetState   string   `yaml:"target_state"`
	ROI           *ROI     `yaml:"roi"`
	MinConfidence float64  `yaml:"min_confidence"`
	Description   string   `yaml:"description"`
	ClickTemplate string   `yaml:"click_template"`
	SaveCursor    bool     `yaml:"save_cursor"`
}

// Action represents something the bot can do.
// Note: priority and sequences are ignored as per requirements.
type Action struct {
	Name           string
	DisplayName    string   `yaml:"display_name"`
	Description    string   `yaml:"description"`
	Enabled        bool     `yaml:"enabled"`
	Button         string   `yaml:"button"`
	State          string   `yaml:"state"`
	ParentStates   []string `yaml:"parent_states"`
	DragDurationMs int      `yaml:"drag_duration_ms"`
	ROI            *ROI     `yaml:"roi"`
	Template       string   `yaml:"template"`
	CooldownS      float64  `yaml:"cooldown_s"`
	Shortcut       string   `yaml:"shortcut"`
	ActionType     string   `yaml:"action_type"`
	DragStart      []float64 `yaml:"drag_start"`
	DragEnd        []float64 `yaml:"drag_end"`
	SaveCursor     bool     `yaml:"save_cursor"`
}

// SequenceStep represents a single step in a sequence.
type SequenceStep struct {
	Action   string  `yaml:"action"`
	TimeoutS float64 `yaml:"timeout_s,omitempty"`
}

// SequenceSchedules represents the schedules for a sequence.
type SequenceSchedules struct {
	EveryDay []string `yaml:"every_day,omitempty"` // list of times like "04:01"
}

// Sequence represents a sequence of actions.
type Sequence struct {
	Name        string             `yaml:"-"`
	Description string             `yaml:"description"`
	Shortcut    string             `yaml:"shortcut"`
	Loop        bool               `yaml:"loop,omitempty"`
	Steps       []SequenceStep     `yaml:"steps"`
	Schedules   *SequenceSchedules `yaml:"schedules,omitempty"`
}
