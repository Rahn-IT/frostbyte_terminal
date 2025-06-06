# frozen term

This is a component for the iced GUI-library which allows displaying an interactive terminal.
It is similar to `iced_term`, but has a few key differences:

- The ANSI-Parser is based on Wezterm
- It allows to connect your own custom datastream
- The Text is completely rendered in iced

## Features
- Connect to any datastream
- ANSI support (uses Wezterm parser) including color support
- Text selection and copy/paste (Ctrl+Shift+C/V)
- scrolling
- Key filtering for custom shortcuts
- resize handling
- focus support (still a bit inconsistent)
- allows for custom monospace fonts (e.g. to embed nerdfonts)

## Usage

### Basic Setup

First, add `frozen_term` to your `Cargo.toml`:

```toml
[dependencies]
frozen_term = "0.4.0"
```

### Creating a Terminal Widget

```rust
use frozen_term::{Terminal, TerminalSize};

// Create a new terminal with specified dimensions
let (terminal, task) = Terminal::new(24, 80); // rows, columns

// Configure the terminal (optional)
let terminal = terminal
    .random_id()                    // Assign a random widget ID
    .font(iced::Font::MONOSPACE)    // Set font
    .padding(10)                    // Set padding around terminal
    .key_filter(|key, modifiers| {  // Filter keys to prevent terminal capture
        // Return true to ignore the key, false to let terminal handle it
        key == &iced::keyboard::Key::Named(iced::keyboard::key::Named::F11)
    });
```

### Integration with Iced Application

```rust
#[derive(Debug, Clone)]
pub enum Message {
    Terminal(frozen_term::Message),
    // ... other messages
}

impl Application for MyApp {
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Terminal(terminal_msg) => {
                let action = self.terminal.update(terminal_msg);
                match action {
                    frozen_term::Action::None => iced::Task::none(),
                    frozen_term::Action::Run(task) => task.map(Message::Terminal),
                    frozen_term::Action::Input(input) => {
                        // Handle terminal input (send to PTY, process, etc.)
                        self.handle_terminal_input(input);
                        iced::Task::none()
                    },
                    frozen_term::Action::Resize(size) => {
                        // Handle terminal resize
                        self.handle_terminal_resize(size);
                        iced::Task::none()
                    },
                }
            },
            // ... handle other messages
        }
    }

    fn view(&self) -> iced::Element<Message> {
        // Include terminal in your view
        self.terminal.view().map(Message::Terminal)
    }
}
```

### Sending Data to Terminal

To display text in the terminal, use the `advance_bytes` method:

```rust
// Send text output to terminal
let output = b"Hello, terminal!\n";
terminal.advance_bytes(output);

// Send ANSI escape sequences for colors, cursor movement, etc.
let colored_text = b"\x1b[31mRed text\x1b[0m\n";
terminal.advance_bytes(colored_text);
```

### Keyboard Shortcuts

The terminal supports the following built-in shortcuts:

- **Ctrl+Shift+C**: Copy selected text to clipboard
- **Ctrl+Shift+V**: Paste text from clipboard
- **Mouse selection**: Click and drag to select text
- **Scrolling**: Use mouse wheel to scroll through terminal history

### Key Filtering

Use the `key_filter` method to prevent the terminal from capturing specific key combinations:

```rust
let terminal = terminal.key_filter(|key, modifiers| {
    // Don't let terminal capture Alt+Tab
    if modifiers.alt() && key == &iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab) {
        return true; // Ignore this key
    }

    // Don't let terminal capture function keys
    if let iced::keyboard::Key::Named(named_key) = key {
        match named_key {
            iced::keyboard::key::Named::F1..=iced::keyboard::key::Named::F12 => true,
            _ => false,
        }
    } else {
        false
    }
});
```

### Complete Example

For a complete working example with PTY integration, see the `frostbyte_term` application in this repo.

If you're missing a feature or found a bug, feel free to create an issue.
