# `hop` Editor

This project aims to provide a terminal-based text and code editor,
taking a lot of inspiration from Sublime Text.


## Features

- select folders to show in the left panel
- explore these folders and open files for edition
- edit multiple files simultaneously via tabs
- cheap syntax highlighting
- intuitive mouse support
- clipboard support
- multi-cursor


## How To Use

See [how-to-use.md](assets/how-to-use.md) in the `assets` folder.


## Using the Clipboard

`hop` relies on external executables for clipboard management.

#### Copying with Ctrl + C

- `hop` will try to copy into `wl-copy`, then into `xclip`, then into `pbcopy`.

#### Pasting with Ctrl + V

- `hop` will try to paste from `wl-paste`, then from `xclip`, then from `pbpaste`.


> When Ctrl+Shift+V is pressed, your terminal should spontaneously
    input characters from a clipboard as if they were pressed;
    `hop` does nothing special in this process.


## Configuration

The path of the configuration file can be specified with the `-c` argument.
By default, this path is `~/.config/hop.toml`.

Please check out the default config file in `assets` from the git repo.
There you will also find a default syntax file.

### TOML Contents

- `background`: hexadecimal color code for the background
- `tree-width`: decimal number of columns for the file tree
- `syntax-file`: path to a syntax file for syntax highligting
- `hide-folders`: list of folders to hide in the file tree
- `hover`: hexadecimal color code for hovering color (tree & tabs)
- `syntax`: map of syntax token types to hexadecimal color codes
