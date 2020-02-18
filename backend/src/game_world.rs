//! Holds state of the game gathered from photon decoded packets.
//! Note: This module is responsible for resolving all inconsistency between photon events and required game events.
//!
//! Inconsistency list:
//!     - player id is different in each zone

use crate::game_events;
use crate::game_events::convert::EventList;
use crate::game_messages;
use crate::id_cache;

#[derive(Debug, Default)]
pub struct GameWorld {
    cache: id_cache::IdCache,
    main_player_id: Option<id_cache::StaticId>,
}

impl GameWorld {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Transforms inconsistent game message into corresponding list of game events
    pub fn consume_message(
        &mut self,
        message: game_messages::Message,
    ) -> Option<Vec<game_events::Events>> {
        match message {
            game_messages::Message::NewCharacter(msg) => {
                let mut result = vec![];

                self.assign_dynamic_id(msg.source, &msg.character_name);
                let static_id = self.get_static_id(msg.source)?;

                if self.main_player_id.is_none() {
                    result.push(game_events::Events::ZoneChange)
                }

                result.append(&mut EventList::from(self.get_intermediate(static_id, msg)?).values());

                Some(result)
            }
            game_messages::Message::Join(msg) => {
                let mut result = vec![];
                
                self.assign_dynamic_id(msg.source, &msg.character_name);
                let static_id = self.get_static_id(msg.source)?;

                if self.main_player_id.is_none() {
                    result.push(game_events::Events::ZoneChange)
                }

                self.main_player_id = Some(static_id);

                result.push(self.get_intermediate(static_id, msg)?.into());

                Some(result)
            }
            game_messages::Message::Leave(msg) => {
                let static_id = self.get_static_id(msg.source)?;
                if let Some(main_player_id) = self.main_player_id {
                    if main_player_id == static_id {
                        return Some(vec![game_events::Events::ZoneChange]);
                    }
                }
                None
            }
            game_messages::Message::HealthUpdate(msg) => {
                let static_id = self.get_static_id(msg.target)?;
                Some(vec![self.get_intermediate(static_id, msg)?.into()])
            }
            game_messages::Message::RegenerationHealthChanged(msg) => {
                let static_id = self.get_static_id(msg.source)?;
                Some(vec![self.get_intermediate(static_id, msg)?.into()])
            }
            game_messages::Message::KnockedDown(msg) => {
                let static_id = self.get_static_id(msg.source)?;
                Some(vec![self.get_intermediate(static_id, msg)?.into()])
            }
            game_messages::Message::UpdateFame(msg) => {
                let static_id = self.get_static_id(msg.source)?;
                Some(vec![self.get_intermediate(static_id, msg)?.into()])
            }
            game_messages::Message::CharacterEquipmentChanged(msg) => {
                let static_id = self.get_static_id(msg.source)?;
                Some(vec![self.get_intermediate(static_id, msg)?.into()])
            }
        }
    }
    fn assign_dynamic_id(&mut self, id: usize, name: &str) {
        let dynamic_id = id_cache::DynamicId::from(id as u32);
        self.cache.save(dynamic_id, name);
    }

    fn get_static_id(&self, id: usize) -> Option<id_cache::StaticId> {
        let dynamic_id = id_cache::DynamicId::from(id as u32);
        self.cache.get_static_id(dynamic_id)
    }

    fn get_intermediate<Msg>(
        &self,
        static_id: id_cache::StaticId,
        msg: Msg,
    ) -> Option<game_events::convert::EventIntermediate<Msg>> {
        Some(game_events::convert::EventIntermediate::new(
            static_id,
            self.cache.get_name(static_id)?,
            msg,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_contains {
        ($container:expr, $value:expr) => {
            assert!(
                format!("{:?}", $container).contains($value),
                format!("{:?} does not contains {}", $container, $value)
            );
        };
    }

    macro_rules! simulate_new_player {
        ($id:expr, $name:expr, $msg:ident) => {
            game_messages::Message::$msg(game_messages::messages::$msg {
                source: $id,
                character_name: $name.to_string(),
                ..Default::default()
            });
        };
    }

    #[test]
    /// game_messages::NewCharacter -> game_events::Events::PlayerAppeared
    fn test_player_appeared() {
        let mut world = GameWorld::new();

        let game_message = simulate_new_player!(1, "TestCharacter", NewCharacter);

        assert!(world.consume_message(game_message.clone()).is_some());
        assert_contains!(
            world.consume_message(game_message.clone()),
            "PlayerAppeared"
        );
    }

    #[test]
    /// game_messages::Join -> Events::PlayerAppeared
    fn test_main_player_appeared() {
        let mut world = GameWorld::new();

        let game_message = simulate_new_player!(1, "TestCharacter", Join);

        assert!(world.consume_message(game_message.clone()).is_some());
        assert_contains!(
            world.consume_message(game_message.clone()),
            "PlayerAppeared"
        );
    }

    #[test]
    /// game_messages::HealthUpdate -> Events::DamageDone | Events::HealthReceived
    fn test_damage_done() {
        let mut world = GameWorld::new();

        let game_message = simulate_new_player!(1, "TestCharacter", Join);
        assert!(world.consume_message(game_message.clone()).is_some());
        assert_contains!(
            world.consume_message(game_message.clone()),
            "PlayerAppeared"
        );

        let target = 1;
        let game_message =
            game_messages::Message::HealthUpdate(game_messages::messages::HealthUpdate {
                target,
                value: -666.0,
                ..Default::default()
            });

        assert!(world.consume_message(game_message.clone()).is_some());
        assert_contains!(world.consume_message(game_message.clone()), "DamageDone");
        assert_contains!(world.consume_message(game_message.clone()), "-666");

        let target = 1;
        let game_message =
            game_messages::Message::HealthUpdate(game_messages::messages::HealthUpdate {
                target,
                value: 666.0,
                ..Default::default()
            });

        assert!(world.consume_message(game_message.clone()).is_some());
        assert_contains!(
            world.consume_message(game_message.clone()),
            "HealthReceived"
        );
        assert_contains!(world.consume_message(game_message.clone()), "666");
    }

    #[test]
    /// game_messages::Leave -> Events::ZoneChange
    fn test_zone_change() {
        let mut world = GameWorld::new();

        let game_message = game_messages::Message::Leave(game_messages::messages::Leave {
            source: 1,
            ..Default::default()
        });

        assert!(world.consume_message(game_message.clone()).is_none());

        let game_message = simulate_new_player!(1, "TestCharacter", Join);

        assert!(world.consume_message(game_message.clone()).is_some());
        assert_contains!(
            world.consume_message(game_message.clone()),
            "PlayerAppeared"
        );

        let game_message = game_messages::Message::Leave(game_messages::messages::Leave {
            source: 1,
            ..Default::default()
        });

        assert_contains!(world.consume_message(game_message.clone()), "ZoneChange");

        let game_message = simulate_new_player!(2, "TestCharacter", NewCharacter);
        assert!(world.consume_message(game_message.clone()).is_some());
        let game_message = game_messages::Message::Leave(game_messages::messages::Leave {
            source: 1,
            ..Default::default()
        });
        assert!(world.consume_message(game_message.clone()).is_none());
    }

    #[test]
    /// game_messages::RegenerationHealthChanged.regeneration_rate -> Events::LeaveCombat
    fn test_combat_leave_via_regeneration_change() {
        let mut world = GameWorld::new();

        let game_message = simulate_new_player!(1, "TestCharacter", Join);
        assert!(world.consume_message(game_message.clone()).is_some());

        let game_message = game_messages::Message::RegenerationHealthChanged(
            game_messages::messages::RegenerationHealthChanged {
                source: 1,
                regeneration_rate: Some(1.0),
                ..Default::default()
            },
        );

        assert_contains!(world.consume_message(game_message.clone()), "LeaveCombat");
    }

    #[test]
    /// game_messages::RegenerationHealthChanged.regeneration_rate -> Events::EnterCombat
    fn test_combat_enter_via_regeneration_change() {
        let mut world = GameWorld::new();

        let game_message = simulate_new_player!(1, "TestCharacter", Join);
        assert!(world.consume_message(game_message.clone()).is_some());

        let game_message = game_messages::Message::RegenerationHealthChanged(
            game_messages::messages::RegenerationHealthChanged {
                source: 1,
                regeneration_rate: None,
                ..Default::default()
            },
        );

        assert_contains!(world.consume_message(game_message.clone()), "EnterCombat");
    }

    #[test]
    /// game_messages::KnockedDown -> Events::EnterCombat
    fn test_combat_enter_via_knockout() {
        let mut world = GameWorld::new();

        let game_message = simulate_new_player!(1, "TestCharacter", Join);
        assert!(world.consume_message(game_message.clone()).is_some());

        let game_message =
            game_messages::Message::KnockedDown(game_messages::messages::KnockedDown {
                source: 1,
                ..Default::default()
            });

        assert_contains!(world.consume_message(game_message.clone()), "LeaveCombat");
    }

    #[test]
    /// game_messages::UpdateFame -> Events::FameUpdate
    fn test_fame_update() {
        let mut world = GameWorld::new();

        let game_message = simulate_new_player!(1, "TestCharacter", Join);
        assert!(world.consume_message(game_message.clone()).is_some());

        let game_message =
            game_messages::Message::UpdateFame(game_messages::messages::UpdateFame {
                source: 1,
                ..Default::default()
            });

        assert_contains!(world.consume_message(game_message.clone()), "UpdateFame");
    }
}
