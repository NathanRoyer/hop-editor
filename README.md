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


## Installation

### Option A: Download the x86_64/linux pre-built binary

Please go to the 'releases' section of this git repository.

### Option B: Build from the rust package registry

1. Install Rust: https://www.rust-lang.org/tools/install
2. Run `cargo install hop-editor`

### Option B: Build from sources

1. Install Rust: https://www.rust-lang.org/tools/install
2. Clone this repository
2. From the newly created directory, run `cargo install --path .`


## How To Use

See [how-to-use.md](assets/how-to-use.md) in the `assets` folder.


## Using the Clipboard

`hop` relies on external executables for clipboard management.

#### Copying with Ctrl + C

- `hop` will try to copy using `wl-copy`, then using `xclip`, then using `pbcopy`.

#### Pasting with Ctrl + V

- `hop` will try to paste using `wl-paste`, then using `xclip`, then using `pbpaste`.


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
