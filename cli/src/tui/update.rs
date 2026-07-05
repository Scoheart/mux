//! The only place the model changes. Pure and synchronous: mutate the model,
//! return side effects to run. No I/O here — that's what makes it unit-testable.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::effect::Effect;
use super::message::Msg;
use super::model::{Counts, Model, Screen};

pub fn update(model: &mut Model, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::Init => return vec![Effect::LoadAll],
        Msg::Tick => model.tick = model.tick.wrapping_add(1),
        Msg::Redraw => {}
        Msg::Loaded {
            registry,
            agents,
            sources,
            installed,
        } => {
            model.loading = false;
            model.counts = Some(Counts {
                registry,
                agents,
                sources,
                installed,
            });
        }
        Msg::Error(e) => model.status = Some(e),
        Msg::Key(k) => return on_key(model, k),
    }
    vec![]
}

fn on_key(model: &mut Model, k: KeyEvent) -> Vec<Effect> {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    match k.code {
        KeyCode::Char('c') if ctrl => model.should_quit = true,
        KeyCode::Char('q') | KeyCode::Esc => model.should_quit = true,
        KeyCode::Char('1') => model.screen = Screen::Registry,
        KeyCode::Char('2') => model.screen = Screen::Sources,
        KeyCode::Char('3') => model.screen = Screen::Agents,
        KeyCode::Tab => model.screen = model.screen.next(),
        KeyCode::BackTab => model.screen = model.screen.prev(),
        KeyCode::Char('r') => {
            model.loading = true;
            return vec![Effect::LoadAll];
        }
        _ => {}
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> Msg {
        Msg::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()))
    }

    #[test]
    fn init_requests_load() {
        let mut m = Model::new();
        let eff = update(&mut m, Msg::Init);
        assert_eq!(eff.len(), 1);
    }

    #[test]
    fn digit_keys_switch_screens() {
        let mut m = Model::new();
        update(&mut m, key('2'));
        assert!(m.screen == Screen::Sources);
        update(&mut m, key('3'));
        assert!(m.screen == Screen::Agents);
    }

    #[test]
    fn tab_cycles_and_wraps() {
        let mut m = Model::new();
        assert!(m.screen == Screen::Registry);
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())));
        assert!(m.screen == Screen::Sources);
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())));
        update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())));
        assert!(m.screen == Screen::Registry); // wrapped
    }

    #[test]
    fn q_and_ctrl_c_quit() {
        let mut m = Model::new();
        update(&mut m, key('q'));
        assert!(m.should_quit);
        let mut m2 = Model::new();
        update(&mut m2, Msg::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(m2.should_quit);
    }

    #[test]
    fn loaded_fills_counts_and_clears_loading() {
        let mut m = Model::new();
        update(
            &mut m,
            Msg::Loaded { registry: 5, agents: 18, sources: 2, installed: 3 },
        );
        assert!(!m.loading);
        assert_eq!(m.counts.unwrap().registry, 5);
    }
}
