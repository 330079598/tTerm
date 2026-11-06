use super::*;

fn style() -> StyleSnapshot {
    StyleSnapshot {
        background_color: [27, 29, 35, 255],
        border_color: [61, 61, 61, 255],
        border_width: 0.0,
        border_radius: 4.0,
        font_family: "system-ui".to_owned(),
        font_size: 14.0,
        font_weight: 400,
        line_height: 16.0,
        color: [204, 204, 204, 255],
        padding: PaddingSnapshot::symmetric(4.0, 12.0),
        declared_background_color: None,
        declared_border_color: None,
    }
}

fn sink() -> LayoutSink {
    LayoutSink::new(
        "example.button-input.default",
        ViewportSnapshot {
            width: 800,
            height: 600,
            device_scale_factor: 2.0,
        },
    )
}

#[test]
fn elements_are_sorted_by_id_and_overwritten_not_appended() {
    let sink = sink();
    for id in ["z.last", "a.first"] {
        sink.record_element(
            id,
            ElementSizing::Fixed,
            BoundsSnapshot::new(1.0, 2.0, 3.0, 4.0),
            style(),
        );
    }
    sink.record_element(
        "a.first",
        ElementSizing::Text,
        BoundsSnapshot::new(5.0, 6.0, 7.0, 8.0),
        style(),
    );
    let document = sink.finish();
    let ids: Vec<&str> = document
        .elements
        .iter()
        .map(|element| element.id.as_str())
        .collect();
    assert_eq!(ids, ["a.first", "z.last"]);
    assert_eq!(document.elements[0].bounds.x, 5.0);
    assert_eq!(document.elements[0].sizing, ElementSizing::Text);
}

#[test]
fn text_regions_are_sorted_by_key() {
    let sink = sink();
    sink.record_text_region("z", BoundsSnapshot::new(0.0, 0.0, 1.0, 1.0));
    sink.record_text_region("a", BoundsSnapshot::new(2.0, 2.0, 3.0, 3.0));
    sink.record_text_region("a", BoundsSnapshot::new(9.0, 9.0, 9.0, 9.0));
    let regions = sink.finish().text_regions;
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].x, 9.0);
    assert_eq!(regions[1].x, 0.0);
}

#[test]
fn json_uses_the_contracts_field_names() {
    let sink = sink();
    sink.record_text_region("label", BoundsSnapshot::new(32.0, 32.0, 80.0, 16.0));
    sink.record_element(
        "example.screen",
        ElementSizing::Fixed,
        BoundsSnapshot::new(0.0, 0.0, 800.0, 600.0),
        style(),
    );
    let json = serde_json::to_value(sink.finish()).expect("layout serialises");
    assert_eq!(json["version"], 2);
    assert_eq!(json["viewport"]["deviceScaleFactor"], 2.0);
    assert_eq!(json["elements"][0]["sizing"], "fixed");
    assert_eq!(json["elements"][0]["style"]["fontFamily"], "system-ui");
    assert_eq!(json["elements"][0]["style"]["padding"]["left"], 12.0);
    assert_eq!(json["textRegions"][0]["width"], 80.0);
}

#[test]
fn declared_colours_are_optional_and_keep_their_names() {
    let mut value = style();
    value.declared_background_color = Some([0, 112, 204, 255]);
    value.declared_border_color = Some([61, 61, 61, 255]);
    let json = serde_json::to_value(value).expect("style serialises");
    assert_eq!(
        json["declaredBackgroundColor"],
        serde_json::json!([0, 112, 204, 255])
    );
    assert_eq!(
        json["declaredBorderColor"],
        serde_json::json!([61, 61, 61, 255])
    );

    let plain = serde_json::to_value(style()).expect("style serialises");
    assert!(plain.get("declaredBackgroundColor").is_none());
    assert!(plain.get("declaredBorderColor").is_none());
}

#[test]
fn the_reference_reader_ignores_extra_fields() {
    let text = r#"{
        "version": 2,
        "screen": "s",
        "viewport": { "width": 10, "height": 10, "deviceScaleFactor": 2 },
        "elements": [],
        "textRegions": [],
        "invokedCommands": ["list_profiles"]
    }"#;
    let document: LayoutDocument = serde_json::from_str(text).expect("reference JSON parses");
    assert_eq!(document.viewport.width, 10);
}

#[test]
fn schema_version_two_is_written() {
    let document = sink().finish();
    assert_eq!(document.version, SCHEMA_VERSION);
    assert_eq!(document.screen, "example.button-input.default");
}
