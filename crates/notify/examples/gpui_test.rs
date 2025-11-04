use gpui::*;

struct HelloWorld;

impl Render for HelloWorld {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        div().child("Hello, World!")
    }
}

fn main() {
    App::new().run(|cx: &mut AppContext| {
        cx.open_window(Default::default(), |cx| HelloWorld);
    });
}
