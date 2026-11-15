//! Tests that a real observing view follows the shared locale entity.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Entity, Render, TestAppContext, Window, div};
use tterm_core::i18n::keys;
use tterm_core::i18n::language::Language;

use super::{Locale, LocaleGlobal};

/// A view that renders the catalogue's subtitle and observes the locale.
struct Subtitle {
    locale: Entity<Locale>,
    text: String,
    notifications: usize,
}

impl Subtitle {
    fn new(locale: Entity<Locale>, cx: &mut Context<Self>) -> Self {
        let mut subtitle = Self {
            locale: locale.clone(),
            text: String::new(),
            notifications: 0,
        };
        subtitle.refresh(cx);
        cx.observe(&locale, |this, _, cx| {
            this.notifications += 1;
            this.refresh(cx);
            cx.notify();
        })
        .detach();
        subtitle
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.text = self
            .locale
            .read(cx)
            .text(keys::APP_SUBTITLE)
            .expect("the embedded subtitle");
    }
}

impl Render for Subtitle {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.text.clone())
    }
}

#[gpui_kit::test]
fn a_language_change_notifies_and_updates_an_observing_view(cx: &mut TestAppContext) {
    let locale = cx.update(|cx| cx.new(|_| Locale::new(Language::En)));
    let subtitle = cx.update(|cx| cx.new(|cx| Subtitle::new(locale.clone(), cx)));

    assert_eq!(
        cx.update(|cx| subtitle.read(cx).text.clone()),
        "Modern Terminal Emulator"
    );

    cx.update(|cx| {
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });

    let (text, notifications) = cx.update(|cx| {
        let subtitle = subtitle.read(cx);
        (subtitle.text.clone(), subtitle.notifications)
    });
    assert_eq!(text, "现代终端模拟器");
    assert_eq!(notifications, 1);

    cx.update(|cx| {
        locale.update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });
    assert_eq!(cx.update(|cx| subtitle.read(cx).notifications), 1);
}

#[gpui_kit::test]
fn startup_installs_one_global_locale_entity(cx: &mut TestAppContext) {
    let locale = cx.update(|cx| {
        let locale = cx.new(|_| Locale::new(Language::En));
        LocaleGlobal::set(cx, locale.clone());
        locale
    });
    let stored = cx.update(|cx| LocaleGlobal::get(cx).map(|locale| locale.entity_id()));

    assert_eq!(stored, Some(locale.entity_id()));
}
