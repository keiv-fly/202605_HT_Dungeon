use std::path::Path;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameConfig {
    pub standard_attack_animation: AttackAnimationConfig,
    pub hero: ActorConfig,
    pub rat: RatConfig,
}

impl GameConfig {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let config: Self = toml::from_str(&contents)
            .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        validate_keyframes(
            "standard_attack_animation.keyframes",
            &self.standard_attack_animation.keyframes,
            None,
        )?;
        self.hero.validate("hero")?;
        self.rat.actor.validate("rat")?;
        if self.rat.actor.attack_animation_keyframes.is_none() {
            validate_keyframes(
                "standard_attack_animation.keyframes",
                &self.standard_attack_animation.keyframes,
                Some(self.rat.actor.attack_animation_seconds),
            )?;
        }
        if self.rat.sight_range <= 0.0 {
            return Err("rat.sight_range must be greater than zero".to_owned());
        }
        if self.rat.lost_sight_seconds < 0.0 {
            return Err("rat.lost_sight_seconds must not be negative".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorConfig {
    pub radius: f32,
    pub max_hp: i32,
    pub movement_speed: f32,
    pub attack_damage_min: i32,
    pub attack_damage_max: i32,
    pub attack_range: f32,
    pub attack_cooldown_seconds: f32,
    pub attack_animation_seconds: f32,
    pub attack_impact_seconds: f32,
    #[serde(default)]
    pub attack_animation_keyframes: Option<Vec<AttackAnimationKeyframe>>,
}

impl ActorConfig {
    fn validate(&self, name: &str) -> Result<(), String> {
        if self.radius <= 0.0 {
            return Err(format!("{name}.radius must be greater than zero"));
        }
        if self.max_hp <= 0 {
            return Err(format!("{name}.max_hp must be greater than zero"));
        }
        if self.movement_speed <= 0.0 {
            return Err(format!("{name}.movement_speed must be greater than zero"));
        }
        if self.attack_damage_min < 0 || self.attack_damage_max < self.attack_damage_min {
            return Err(format!(
                "{name}.attack damage values must be non-negative and ordered"
            ));
        }
        if self.attack_range <= 0.0 {
            return Err(format!("{name}.attack_range must be greater than zero"));
        }
        if self.attack_cooldown_seconds < 0.0 {
            return Err(format!(
                "{name}.attack_cooldown_seconds must not be negative"
            ));
        }
        if self.attack_animation_seconds <= 0.0 {
            return Err(format!(
                "{name}.attack_animation_seconds must be greater than zero"
            ));
        }
        if self.attack_impact_seconds < 0.0
            || self.attack_impact_seconds > self.attack_animation_seconds
        {
            return Err(format!(
                "{name}.attack_impact_seconds must be within the animation duration"
            ));
        }
        if let Some(keyframes) = &self.attack_animation_keyframes {
            validate_keyframes(
                &format!("{name}.attack_animation_keyframes"),
                keyframes,
                Some(self.attack_animation_seconds),
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackAnimationConfig {
    pub keyframes: Vec<AttackAnimationKeyframe>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RatConfig {
    #[serde(flatten)]
    pub actor: ActorConfig,
    pub sight_range: f32,
    pub lost_sight_seconds: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackAnimationKeyframe {
    pub time_seconds: f32,
    pub offset_pixels: f32,
}

fn validate_keyframes(
    name: &str,
    keyframes: &[AttackAnimationKeyframe],
    max_duration: Option<f32>,
) -> Result<(), String> {
    if keyframes.is_empty() {
        return Err(format!("{name} must not be empty"));
    }

    let mut previous_time = f32::NEG_INFINITY;
    for keyframe in keyframes {
        if keyframe.time_seconds < 0.0 || keyframe.time_seconds < previous_time {
            return Err(format!("{name} must be sorted and non-negative"));
        }
        if max_duration.is_some_and(|duration| keyframe.time_seconds > duration) {
            return Err(format!("{name} must fit within the animation duration"));
        }
        previous_time = keyframe.time_seconds;
    }

    Ok(())
}
