# Design

## Application

- General application state:
  - List of all messages (sorted by timestamp)
    - Maybe also structures to make it easier to get all messages in a room?
  - Configs

## UI

- main view consists of:
  - message list (takes up most of screen)
  - status bar (one line)
  - command line (replaces status bar when activated?)
  - message composition box (when activated; resizable)
  - left sidebar: room list (optional, resizable)
  - right sidebar: info pop-up (optional, resizable)

## Matrix

- Matrix logic should be abstracted from UI logic
- need to handle many different types of events

## Common

- shared crate to provide interfaces for abstracting messages and anything else
  that might want to be abstracted
