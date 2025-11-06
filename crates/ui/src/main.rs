use gpui::*;
use gpui_component::{ActiveTheme as _, Root, init};

mod assets;
mod services;
mod theme;
mod window;
mod workspace;

use assets::Assets;
use theme::change_color_mode;
use window::get_window_options;
use workspace::Workspace;

fn main() {
    env_logger::init();

    let application = Application::new().with_assets(Assets);

    application.run(|cx: &mut App| {
        let window_options = get_window_options(cx);
        cx.open_window(window_options, |win, cx| {
            init(cx);
            change_color_mode(cx.theme().mode, win, cx);

            let workspace_view = Workspace::view(win, cx);
            cx.new(|cx| Root::new(workspace_view.into(), win, cx))
        })
        .unwrap();
    });
}
