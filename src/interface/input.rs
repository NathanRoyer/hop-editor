use super::*;

#[derive(Copy, Clone, Debug)]
pub enum UserInput {
    Quit(bool),
    Save,
    CloseTab(Option<u16>),
    NextTab(bool),
    Insert(char),
    InsertTab,
    CarriageReturn,
    CodeSeek(u16, u16, bool),
    CodeDrag(u16, u16),
    Reveal,
    TreeClick(u16),
    CursorClick(u16),
    TreeHover(u16),
    TabHover(u16),
    CursorHover(u16),
    ClearHover,
    TabClick(u16),
    Backspace(bool),
    Find,
    Paste,
    Copy,
    Cut,
    Undo,
    Redo,
    SeekLineStart(bool),
    SeekLineEnd(bool),
    PanelResize(bool),
    Scroll(isize),
    Resize(u16, u16),
    HorizontalJump(isize, bool),
    VerticalJump(isize, bool),
    ContextMenu(Location, u16, u16),
    AutoSelect,
    SelectAll,
    NoOp,
}

pub enum ResizeEvent {
    Drag(u16),
    Right,
    Left,
    Stop,
    NoOp,
}

#[derive(Copy, Clone, Debug)]
pub enum Location {
    PanelSep,
    Menu,
    MenuEdge,
    TreeRow(u16),
    Cursors(u16),
    Tab(u16),
    LineNo(u16),
    Code(u16, u16),
}

impl Interface {
    fn cursor_pos(&self, x: u16, y: u16, num_cursors: u16) -> Location {
        let cursors_y = self.height.saturating_sub(num_cursors + 1);
        let code_x = self.panel_width + (LN_WIDTH as u16) + 3;
        let tree_y = MENU_HEIGHT + 1;

        if x == self.panel_width {
            Location::PanelSep
        } else if x < self.panel_width {
            if y < MENU_HEIGHT {
                Location::Menu
            } else if y == cursors_y {
                Location::MenuEdge
            } else if y > cursors_y {
                Location::Cursors(y - (cursors_y + 1))
            } else if y >= tree_y {
                Location::TreeRow(y - tree_y)
            } else {
                Location::MenuEdge
            }
        } else if y < 3 {
            Location::Tab(x - self.panel_width - 1)
        } else if x < code_x {
            Location::LineNo(y - 3)
        } else {
            Location::Code(x - code_x, y - 3)
        }
    }

    pub fn read_event(&self, num_cursors: u16) -> UserInput {
        let code_height = self.code_height() as isize;
        let event = read().unwrap();

        let fallback = || {
            crate::alert!("unassigned action:\n- event: {event:?}\n");
            UserInput::NoOp
        };

        match &event {
            Event::Key(e) if e.is_release() => UserInput::NoOp,
            Event::Key(e) => {
                let shift = e.modifiers.contains(KeyModifiers::SHIFT);

                if e.modifiers.contains(KeyModifiers::CONTROL) {
                    match e.code {
                        KeyCode::Right => UserInput::HorizontalJump(10, shift),
                        KeyCode::Left => UserInput::HorizontalJump(-10, shift),
                        KeyCode::Char('d') => UserInput::AutoSelect,
                        KeyCode::Char('a') => UserInput::SelectAll,
                        KeyCode::Char('w') => UserInput::CloseTab(None),
                        KeyCode::Char('o') => UserInput::Reveal,
                        KeyCode::Char('q') => UserInput::Quit(true),
                        KeyCode::Char('s') => UserInput::Save,
                        KeyCode::Char('z') => UserInput::Undo,
                        KeyCode::Char('y') => UserInput::Redo,
                        KeyCode::Char('f') => UserInput::Find,
                        KeyCode::Char('v') => UserInput::Paste,
                        KeyCode::Char('c') => UserInput::Copy,
                        KeyCode::Char('x') => UserInput::Cut,
                        KeyCode::Home => UserInput::PanelResize(!shift),
                        KeyCode::Down => UserInput::Scroll(1),
                        KeyCode::Up => UserInput::Scroll(-1),
                        _ => fallback(),
                    }
                } else {
                    match e.code {
                        KeyCode::PageDown if shift => UserInput::NextTab(true),
                        KeyCode::PageUp if shift => UserInput::NextTab(false),
                        KeyCode::PageDown => UserInput::Scroll(code_height),
                        KeyCode::PageUp => UserInput::Scroll(-code_height),
                        KeyCode::Right => UserInput::HorizontalJump(1, shift),
                        KeyCode::Left => UserInput::HorizontalJump(-1, shift),
                        KeyCode::Down => UserInput::VerticalJump(1, shift),
                        KeyCode::Up => UserInput::VerticalJump(-1, shift),
                        KeyCode::Backspace => UserInput::Backspace(false),
                        KeyCode::Delete => UserInput::Backspace(true),
                        KeyCode::Enter => UserInput::CarriageReturn,
                        KeyCode::Char(c) => UserInput::Insert(c),
                        KeyCode::Home => UserInput::SeekLineStart(shift),
                        KeyCode::End => UserInput::SeekLineEnd(shift),
                        KeyCode::Tab => UserInput::InsertTab,
                        KeyCode::Esc => UserInput::Quit(false),
                        _ => fallback(),
                    }
                }
            },
            Event::Mouse(e) => {
                use {MouseEventKind::*, MouseButton::*};
                let ctrl = e.modifiers.contains(KeyModifiers::CONTROL);
                let pos = self.cursor_pos(e.column, e.row, num_cursors);
                let context_menu = UserInput::ContextMenu(pos, e.column, e.row);

                let mouse_fallback = || {
                    // crate::alert!("unassigned action:\n- event: {e:?}\n- pos: {pos:?}");
                    UserInput::NoOp
                };

                match pos {
                    Location::Code(x, y) => match e.kind {
                        ScrollDown => UserInput::Scroll(1),
                        ScrollUp => UserInput::Scroll(-1),
                        Down(Left) => UserInput::CodeSeek(x, y, ctrl),
                        Up(_) => UserInput::NoOp,
                        Drag(Left) => UserInput::CodeDrag(x, y),
                        Moved => UserInput::ClearHover,
                        _ => mouse_fallback(),
                    },
                    Location::TreeRow(y) => match e.kind {
                        ScrollDown => UserInput::Scroll(1),
                        ScrollUp => UserInput::Scroll(-1),
                        Up(_) => UserInput::NoOp,
                        Down(Left) => UserInput::TreeClick(y),
                        Down(Right) => context_menu,
                        Moved => UserInput::TreeHover(y),
                        Drag(Left) => UserInput::NoOp,
                        _ => mouse_fallback(),
                    },
                    Location::Cursors(y) => match e.kind {
                        Up(_) => UserInput::NoOp,
                        Down(Left) => UserInput::CursorClick(y),
                        ScrollDown => UserInput::Scroll(1),
                        ScrollUp => UserInput::Scroll(-1),
                        Moved => UserInput::CursorHover(y),
                        Drag(Left) => UserInput::NoOp,
                        _ => mouse_fallback(),
                    },
                    Location::Tab(x) => match e.kind {
                        Up(_) => UserInput::NoOp,
                        Down(Left) => UserInput::TabClick(x),
                        Down(Middle) => UserInput::CloseTab(Some(x)),
                        Down(Right) => context_menu,
                        Moved => UserInput::TabHover(x),
                        Drag(Left) => UserInput::NoOp,
                        _ => mouse_fallback(),
                    },
                    Location::LineNo(_y) => match e.kind {
                        Up(_) => UserInput::NoOp,
                        Moved => UserInput::ClearHover,
                        Drag(Left) => UserInput::NoOp,
                        _ => mouse_fallback(),
                    },
                    Location::PanelSep => match e.kind {
                        Down(Left) => UserInput::PanelResize(false),
                        Moved => UserInput::ClearHover,
                        Up(_) => UserInput::NoOp,
                        _ => mouse_fallback(),
                    },
                    Location::MenuEdge => match e.kind {
                        Up(_) => UserInput::NoOp,
                        Moved => UserInput::ClearHover,
                        _ => mouse_fallback(),
                    },
                    Location::Menu => match e.kind {
                        Up(_) => UserInput::NoOp,
                        Moved => UserInput::ClearHover,
                        _ => mouse_fallback(),
                    },
                }
            },
            Event::Resize(w, h) => UserInput::Resize(*w, *h),
            _other => fallback(),
        }
    }

    pub fn get_panel_width(&self) -> u16 {
        self.panel_width
    }

    pub fn panel_width_op<F: Fn(u16) -> u16 + ?Sized>(&mut self, op: &F) {
        self.panel_width = op(self.panel_width);
    }

    pub fn panel_resize_event(&self) -> ResizeEvent {
        match read().unwrap() {
            Event::Key(e) => {
                match e.code {
                    KeyCode::Right => ResizeEvent::Right,
                    KeyCode::Left => ResizeEvent::Left,
                    KeyCode::Esc => ResizeEvent::Stop,
                    _ => ResizeEvent::NoOp,
                }
            },
            Event::Mouse(e) => {
                use {MouseEventKind::*, MouseButton::*};
                match e.kind {
                    Drag(Left) => ResizeEvent::Drag(e.column),
                    Moved => ResizeEvent::Stop,
                    Up(_) => ResizeEvent::Stop,
                    _ => ResizeEvent::NoOp,
                }
            },
            _other => ResizeEvent::NoOp,
        }
    }
}
