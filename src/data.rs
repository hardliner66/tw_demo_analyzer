use serde::Serialize;
use twsnap::{enums, items::Tee};

use fixed::types::{I24F8, I27F5};
pub type PositionPrecision = I27F5;
pub type VelocityPrecision = I24F8;
pub type AnglePrecision = I24F8;

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub x: PositionPrecision,
    pub y: PositionPrecision,
}

impl From<twsnap::Position> for Position {
    fn from(value: twsnap::Position) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Velocity {
    pub x: VelocityPrecision,
    pub y: VelocityPrecision,
}

impl From<twsnap::Velocity> for Velocity {
    fn from(value: twsnap::Velocity) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Direction {
    Left,
    None,
    Right,
}

impl From<enums::Direction> for Direction {
    fn from(value: enums::Direction) -> Self {
        match value {
            enums::Direction::Left => Direction::Left,
            enums::Direction::None => Direction::None,
            enums::Direction::Right => Direction::Right,
        }
    }
}

#[derive(Serialize)]
pub enum HookState {
    Retracted,
    Idle,
    RetractStart,
    Retracting,
    RetractEnd,
    Flying,
    Grabbed,
}

impl From<enums::HookState> for HookState {
    fn from(value: enums::HookState) -> Self {
        match value {
            enums::HookState::Retracted => HookState::Retracted,
            enums::HookState::Idle => HookState::Idle,
            enums::HookState::RetractStart => HookState::RetractStart,
            enums::HookState::Retracting => HookState::Retracting,
            enums::HookState::RetractEnd => HookState::RetractEnd,
            enums::HookState::Flying => HookState::Flying,
            enums::HookState::Grabbed => HookState::Grabbed,
        }
    }
}

#[derive(Serialize)]
pub enum ActiveWeapon {
    Hammer,
    Pistol,
    Shotgun,
    Grenade,
    Rifle,
    Ninja,
}

impl From<enums::ActiveWeapon> for ActiveWeapon {
    fn from(value: enums::ActiveWeapon) -> Self {
        match value {
            enums::ActiveWeapon::Hammer => ActiveWeapon::Hammer,
            enums::ActiveWeapon::Pistol => ActiveWeapon::Pistol,
            enums::ActiveWeapon::Shotgun => ActiveWeapon::Shotgun,
            enums::ActiveWeapon::Grenade => ActiveWeapon::Grenade,
            enums::ActiveWeapon::Rifle => ActiveWeapon::Rifle,
            enums::ActiveWeapon::Ninja => ActiveWeapon::Ninja,
        }
    }
}

#[derive(Serialize)]
pub enum Emote {
    Normal,
    Pain,
    Happy,
    Surprise,
    Angry,
    Blink,
}

impl From<enums::Emote> for Emote {
    fn from(value: enums::Emote) -> Self {
        match value {
            enums::Emote::Normal => Emote::Normal,
            enums::Emote::Pain => Emote::Pain,
            enums::Emote::Happy => Emote::Happy,
            enums::Emote::Surprise => Emote::Surprise,
            enums::Emote::Angry => Emote::Angry,
            enums::Emote::Blink => Emote::Blink,
        }
    }
}

#[derive(Serialize)]
pub struct Inputs {
    pub tick: i32,
    pub pos: Position,
    pub vel: Velocity,

    pub angle: AnglePrecision,
    pub direction: Direction,

    pub hook_state: HookState,
    pub hook_tick: i32,

    pub hook_pos: Position,
    pub hook_direction: Velocity,

    pub health: i32,
    pub armor: i32,
    pub ammo_count: i32,
    pub weapon: ActiveWeapon,
    pub emote: Emote,
    pub attack_tick: i32,

    // DDNetCharacter
    pub freeze_end: i32,
    pub jumps: i32,
    pub tele_checkpoint: i32,
    pub strong_weak_id: i32,
    pub jumped_total: i32,
    pub ninja_activation_tick: i32,
    pub target: Position,
}

impl From<&Tee> for Inputs {
    fn from(value: &Tee) -> Self {
        Self {
            tick: (value.tick.seconds() * 50.0) as i32,
            pos: value.pos.into(),
            vel: value.vel.into(),
            angle: value.angle,
            direction: value.direction.into(),
            hook_state: value.hook_state.into(),
            hook_tick: value.hook_tick.ticks(),
            hook_pos: value.hook_pos.into(),
            hook_direction: value.hook_direction.into(),
            health: value.health,
            armor: value.armor,
            ammo_count: value.ammo_count,
            weapon: value.weapon.into(),
            emote: value.emote.into(),
            attack_tick: (value.attack_tick.seconds() * 50.0) as i32,
            freeze_end: (value.freeze_end.seconds() * 50.0) as i32,
            jumps: value.jumps,
            tele_checkpoint: value.tele_checkpoint,
            strong_weak_id: value.strong_weak_id,
            jumped_total: value.jumped_total,
            ninja_activation_tick: (value.ninja_activation_tick.seconds() * 50.0) as i32,
            target: value.target.into(),
        }
    }
}
