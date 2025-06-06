# frozen term

This is a component for the iced GUI-library which allows displaying an interactive terminal.
It is similar to `iced_term`, but has a few key differences:

- The ANSI-Parser is based on Wezterm
- It allows to connect your own custom datastream
- The Text is completely rendered in iced

## Features
- Can be connected to any datastream
- Copy and paste using Ctrl + Shift + C/V
- Color Support
- Scrolling

If you're missing a feature or found a bug, feel free to create an issue.
