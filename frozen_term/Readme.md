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

## iced 0.13

If you need support for iced 0.13, please create an issue. As the code started deviating more,
the feature flag for 0.13 became harder and harder to maintain.

It should still be relatively easy for me to add back support for iced 0.13 in a branch,
but I don't want to go through that trouble unless someone actually needs it.

## Usage

### Local terminal

If you don't need to connect your own datastream from e.g. a serial port or a remote connection, it's recommended to use the `LocalTerminal`, which you can enable via the `local-terminal` feature.

You can find a minimal terminal for a local terminal in the examples folder.

### Adding as Dependency

First, add `frozen_term` to your `Cargo.toml`.
Don't worry about the patches in the workspace file - these only apply for binaries in the workspace.

```toml
[dependencies]
frozen_term = { git = "https://github.com/rahn-it/frostbyte_terminal.git", features = [
    "local-terminal",
] }
iced = { git = "https://github.com/iced-rs/iced.git", features = ["wgpu"] }
```

Please ensure that your iced entry has neither a branch nor a revision selected. Otherwise you will face type collisions.
If you want to use a custom version of iced, use cargo patching like this:

```toml
[patch."https://github.com/iced-rs/iced.git"]
iced = { git = "https://github.com/acul009/iced.git" }
iced_runtime = { git = "https://github.com/acul009/iced.git" }
iced_core = { git = "https://github.com/acul009/iced.git" }
iced_program = { git = "https://github.com/acul009/iced.git" }
iced_renderer = { git = "https://github.com/acul009/iced.git" }
iced_futures = { git = "https://github.com/acul009/iced.git" }
iced_graphics = { git = "https://github.com/acul009/iced.git" }
iced_debug = { git = "https://github.com/acul009/iced.git" }
iced_devtools = { git = "https://github.com/acul009/iced.git" }
iced_widget = { git = "https://github.com/acul009/iced.git" }
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
