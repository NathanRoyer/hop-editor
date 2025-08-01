# `hop` Editor

This project aims to provide a terminal-based text and code editor,
taking a lot of inspiration from Sublime Text.


## Features

- select folders to show in the left panel
- explore these folders and open files for edition
- edit multiple files simultaneously via tabs
- syntax highlighting in edited files
- intuitive mouse support
- multi-cursor


## How To Use

### Quitting

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Escape               | Quit `hop` or filetree keyboard mode |
| Ctrl + Q             | Quit `hop`                           |

### Cursors

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Click                | Replace cursors with a new one       |
| Drag                 | Replace cursors with a selection     |
| Ctrl + Click         | Add a cursor                         |
| Ctrl + Drag          | Add a selection                      |
| Ctrl + D             | Auto-Select                          |
| Arrows               | Move all cursors                     |
| Ctrl + Right         | Move all cursors 10 characters ahead |
| Ctrl + Left          | Move all cursors 10 characters back  |

### Edition

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + Z             | Undo                                 |
| Ctrl + Y             | Redo                                 |
| Ctrl + X             | Cut                                  |
| Ctrl + C             | Copy                                 |
| Ctrl + V             | Paste via utility                    |
| Ctrl + Shift + V     | Paste via terminal                   |

### Scrolling

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + Down          | Scroll down one line                 |
| Ctrl + Up            | Scroll up one line                   |
| Page Down            | Scroll down one page                 |
| Page Up              | Scroll up one page                   |
| Mouse Wheel          | Scroll                               |

### Tabs

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + S             | Save                                 |
| Ctrl + F             | Find in tab                          |
| Shift + Page Down    | Switch to previous tab               |
| Shift + Page Up      | Switch to next tab                   |
| Ctrl + W             | Close Tab                            |

### File Tree Mode

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + O             | Switch to file tree keyboard mode    |
| Escape or Click      | Return to normal keyboard mode       |
| Enter or Space       | Open file / (un)fold directory       |
| Left and Right       | Enter and Leave directories          |
| Up and Down          | Navigate in tree                     |
| Ctrl + F             | Find in tree                         |



## Using the Clipboard

#### Copying with Ctrl + C

- `hop` will try to copy into `wl-copy`, then into `xclip`, then into `pbcopy`.
- `hop` will also instruct your terminal to copy the selected text into a clipboard.

#### Pasting

- When Ctrl+V is pressed, `hop` will try to paste from `wl-paste`,
    then from `xclip`, then from `pbpaste`.

- When Ctrl+Shift+V is pressed, your terminal should spontaneously
    input characters from a clipboard as if they were pressed;
    `hop` does nothing special in this process.


## Configuration

The path of the configuration file can be specified with the `-c` argument.
By default, this path is `~/.config/hop.toml`.

### TOML Contents

- `background-color`: hexadecimal color code or "default" (default)
- `tree-width`: decimal number of columns (default: 40)
- `ascii-only`: `true` or `false` (default: `false`)


## To-Do List

- copy / cut / paste
- undo / redo (C-z, C-y)
- find (in tab or tree mode) (C-f)
- find and replace
- home / end
