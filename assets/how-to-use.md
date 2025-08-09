# How To Use

## Startup

```sh
hop project-a/ project-b/ project-a/src/main.rs
```

In the example above, hop is asked to:
- add two folders in the file tree: `project-a/` & `project-b/`
- open one file for edition: `project-a/src/main.rs`

All files and directories passed as arguments must be valid, existing paths.

## Quitting

If some files have unsaved modifications, `hop` will give you a warning
before quitting.

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Escape               | Quit `hop` or filetree keyboard mode |
| Ctrl + Q             | Quit `hop`                           |

## Cursors

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Click                | Replace cursors with a new one       |
| Drag                 | Replace cursors with a selection     |
| Ctrl + Click         | Add a cursor                         |
| Ctrl + Drag          | Add a selection                      |
| Ctrl + A             | Select All                           |
| Ctrl + D             | Auto-Select                          |
| Arrows               | Move all cursors                     |
| Ctrl + Right         | Move all cursors 10 characters ahead |
| Ctrl + Left          | Move all cursors 10 characters back  |

## Edition

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + Z             | Undo                                 |
| Ctrl + Y             | Redo All                             |
| Ctrl + X             | Cut                                  |
| Ctrl + C             | Copy                                 |
| Ctrl + V             | Paste                                |

## Scrolling

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + Down          | Scroll down one line                 |
| Ctrl + Up            | Scroll up one line                   |
| Page Down            | Scroll down one page                 |
| Page Up              | Scroll up one page                   |
| Mouse Wheel          | Scroll                               |

## Tabs

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + S             | Save                                 |
| Ctrl + F             | Find in tab                          |
| Shift + Page Down    | Switch to previous tab               |
| Shift + Page Up      | Switch to next tab                   |
| Ctrl + W             | Close Tab                            |
| Middle Click         | Close Tab                            |

## File Tree Mode

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + O             | Switch to file tree keyboard mode    |
| Escape or Click      | Return to normal keyboard mode       |
| Enter or Space       | Open file / (un)fold directory       |
| Left and Right       | Enter and Leave directories          |
| Up and Down          | Navigate in tree                     |
| Ctrl + F             | Find in tree                  [todo] |

## Others

| User Input           | Action                               |
|----------------------|--------------------------------------|
| Ctrl + Home          | Toggle Left Panel                    |
| Ctrl + Shift + Home  | Resize Left Panel (using arrows)     |
| Click + Drag Bar     | Resize Left Panel (using mouse)      |