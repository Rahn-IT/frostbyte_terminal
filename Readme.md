# Frostbyte Terminal

I've always loved Yakuake on my KDE Desktop,
but I've been missing it dearly on other DE's and especially on windows.
Since I had to build a terminal widget for the [rust ui framework iced](https://iced.rs/) anyway,
I needed a test application. And after building the terminal itself,
I just went ahead and kept working on it a bit.

There's still a bit of work until it's ready for other users.
In the meantime, feel free to check out the code and maybe try it out yourself.

## Architecture

Frostbyte uses the [rust ui framework iced.](https://iced.rs/)

To understand the code, you'll need to be familiar with the iced basics.

### async_pty

This is just a badly made async wrapper around `portable-pty`.

### frozen_term

This is the iced widget / component pattern.
It handles parsing of the given terminal output and can generate a view to display the terminal.

The widget is designed to be plugged into any pty backend, be it serial, over the network or locally like Frostbyte does.

### frostbyte_term

Frostbyte is the actual application which uses the widget.

It has a submodule `local_terminal` which handles the creation of the pty and
facilitates the required communication between the component and the pty.
If you want to see an example of how to use `frozen_term`, this is the place to look.
