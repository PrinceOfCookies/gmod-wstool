use eframe::egui;

pub fn tab_button(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let button_size = egui::vec2(120.0, 32.0);
    ui.add_sized(
        button_size,
        egui::Button::new(egui::RichText::new(label).size(15.0))
            .selected(selected)
            .corner_radius(6),
    )
    .clicked()
}
