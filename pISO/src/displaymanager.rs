use std::collections::BTreeMap;
use bitmap;
use display;
use error::{Result, ResultExt};
use render;
use std::sync::{Arc, Mutex};
use input;

pub type WindowId = u32;

pub enum Position {
    Fixed(usize, usize),
    Relative(usize, usize),
    Normal,
}

pub struct Window {
    pub position: Position,
    pub size: (usize, usize),
    pub z: u32,
    pub focus: bool,
    pub id: WindowId,
    pub parent: WindowId,
}

pub struct DisplayManager {
    display: display::Display,
    windows: BTreeMap<WindowId, Window>,
    nextid: u32,
}

impl DisplayManager {
    pub fn new() -> Result<Arc<Mutex<DisplayManager>>> {
        let mut disp = display::Display::new().chain_err(|| "Failed to create display")?;
        disp.on().chain_err(|| "Failed to activate display")?;

        Ok(Arc::new(Mutex::new(DisplayManager {
            display: disp,
            windows: BTreeMap::new(),
            nextid: 1,
        })))
    }

    pub fn root(&self) -> WindowId {
        0
    }

    pub fn add_child(&mut self, parent: WindowId, pos: Position) -> Result<WindowId> {
        let id = self.nextid;
        self.nextid += 1;

        self.windows.insert(
            id,
            Window {
                position: pos,
                id: id,

                // Default the size to 0, 0 and set during the render to whatever
                // the size actually is
                size: (0, 0),

                //TODO: this should probably be the parent z + 1
                z: 0,
                focus: false,
                parent: parent,
            },
        );

        Ok(id)
    }

    pub fn remove_child(&mut self, id: WindowId) -> Result<()> {
        //TODO: remove children recursively
        self.windows.remove(&id);
        Ok(())
    }

    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(&id)
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.get_mut(&id)
    }

    fn children(&self, id: WindowId) -> Vec<WindowId> {
        self.windows
            .iter()
            .filter_map(|(&winid, ref window)| {
                if window.parent == id {
                    Some(winid)
                } else {
                    None
                }
            })
            .collect()
    }

    fn parent_window(&self, window: &Window) -> Option<&Window> {
        self.windows
            .values()
            .filter(|win| win.id == window.parent)
            .next()
    }

    fn calculate_position(&self, window: &Window) -> (usize, usize) {
        let parent_win = self.parent_window(window);
        let parent_size = parent_win.map(|win| win.size).unwrap_or((0, 0));
        let parent_pos = parent_win
            .map(|win| self.calculate_position(win))
            .unwrap_or((0, 0));

        match window.position {
            Position::Fixed(x, y) => (x, y),
            Position::Relative(x_off, y_off) => {
                (parent_pos.0 + x_off, parent_pos.1 + parent_size.1 + y_off)
            }
            Position::Normal => (parent_pos.0, parent_pos.1 + parent_size.1),
        }
    }

    pub fn render(&mut self, root: &Widget) -> Result<()> {
        fn render_window(
            manager: &mut DisplayManager,
            base: &mut bitmap::Bitmap,
            widget: &Widget,
        ) -> Result<()> {
            //TODO: make the less terrible
            let bmap = {
                let mut window = manager
                    .get_mut(widget.windowid())
                    .ok_or(format!("failed to find window id={}", widget.windowid()))?;
                let bmap = widget.render(window)?;
                window.size = (bmap.width(), bmap.height());
                bmap
            };
            {
                let window = manager
                    .get(widget.windowid())
                    .ok_or(format!("failed to find window id={}", widget.windowid()))?;

                let pos = manager.calculate_position(window);
                base.blit(bmap, pos);
            }

            for child in widget.children() {
                render_window(manager, base, child)?
            }

            Ok(())
        };

        let mut bitmap = bitmap::Bitmap::new(0, 0);
        render_window(self, &mut bitmap, root)?;

        println!(
            "Update display with bitmap: {} by {}",
            bitmap.width(),
            bitmap.height()
        );
        self.display.update(bitmap)?;

        Ok(())
    }
}

pub trait Widget: render::Render + input::Input {
    fn children(&self) -> Vec<&Widget>;
    fn windowid(&self) -> WindowId;
}
