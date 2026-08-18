import re

with open('config/states.yaml', 'r') as f:
    content = f.read()

# 1. Pad single decimal coordinates to two decimals
content = re.sub(r'(xmin: \d+\.\d)\n', r'\g<1>0\n', content)
content = re.sub(r'(xmax: \d+\.\d)\n', r'\g<1>0\n', content)
content = re.sub(r'(ymin: \d+\.\d)\n', r'\g<1>0\n', content)
content = re.sub(r'(ymax: \d+\.\d)\n', r'\g<1>0\n', content)

# 2. Fix RADAR_TASKS_BUTTON ROI
old_radar = """  target_state: RADAR_TASKS
  template: roi/RADAR_TASKS_BUTTON/expected.png
  roi:
    xmin: 0.55
    xmax: 0.67
    ymin: 0.33
    ymax: 0.45"""
new_radar = """  target_state: RADAR_TASKS
  template: roi/RADAR_TASKS_BUTTON/expected.png
  roi:
    xmin: 0.43
    xmax: 0.56
    ymin: 0.56
    ymax: 0.68"""
content = content.replace(old_radar, new_radar)

# 3. Fix BASE_BUTTON -> AREA_BASE_BUTTON and WORLD_MAP_BASE_BUTTON
old_base_button = """- id: BASE_BUTTON
  display_name: Base Button
  parent_states:
  - AREA
  - WORLD_MAP
  target_state: BASE
  template: roi/BASE_BUTTON/expected.png
  roi:
    xmin: 0.88
    xmax: 1.00
    ymin: 0.86
    ymax: 1.00
  min_confidence: 0.90
  save_cursor: false
  description: Button to return to the Base from Area or World Map."""
new_base_buttons = """- id: AREA_BASE_BUTTON
  display_name: Area Base Button
  parent_states:
  - AREA
  target_state: BASE
  template: roi/AREA_BASE_BUTTON/expected.png
  roi:
    xmin: 0.88
    xmax: 1.00
    ymin: 0.86
    ymax: 1.00
  min_confidence: 0.90
  save_cursor: false
  description: Button to return to the Base from Area.
- id: WORLD_MAP_BASE_BUTTON
  display_name: World Map Base Button
  parent_states:
    - WORLD_MAP
  target_state: BASE
  template: roi/WORLD_MAP_BASE_BUTTON/expected.png
  roi:
    xmin: 0.88
    xmax: 1.00
    ymin: 0.86
    ymax: 1.00
  min_confidence: 0.80
  save_cursor: false
  description: Button to return to the Base from World Area."""
content = content.replace(old_base_button, new_base_buttons)

# 4. Fix clail_all.png to claim_all.png
content = content.replace("click_template: clail_all.png", "click_template: claim_all.png")

# 5. Add new actions at the bottom
new_actions = """
- name: base_view_from_area
  description: Go to base view from area
  template: roi/AREA_BASE_BUTTON/expected.png
  enabled: false
  button: AREA_BASE_BUTTON
  state: BASE
  parent_states:
    - AREA
  action_type: click_template
  click_template: null
  roi: null
  coords: null
  key_name: null
  min_confidence: 0.80
  cooldown_s: 3.0
  priority: 3
  save_cursor: false
  shortcut: ctrl+b

- name: base_view_from_world_map
  description: Go to base view from world map
  template: roi/WORLD_MAP_BASE_BUTTON/expected.png
  enabled: false
  button: WORLD_MAP_BASE_BUTTON
  state: BASE
  parent_states:
    - WORLD_MAP
  action_type: click_template
  roi: null
  coords: null
  key_name: null
  min_confidence: 0.80
  cooldown_s: 3.0
  priority: 3
  save_cursor: false
  shortcut: ctrl+b
"""
content += new_actions

with open('config/states.yaml', 'w') as f:
    f.write(content)

