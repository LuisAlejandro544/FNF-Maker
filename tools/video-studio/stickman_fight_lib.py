"""
Archivo: tools/video-studio/stickman_fight_lib.py
Propósito: Motor modular de coreografía, cinemática 2D, partículas, cámara y
           síntesis de audio DSP (música + efectos sincronizados a 48 kHz estéreo)
           para el combate de 20 segundos entre Stickman Azul y Stickman Rojo.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from typing import Dict, List, Tuple
import numpy as np


# Pose articular normalizada en espacio local del personaje (unidades tipo píxel)
# Cada ángulo está en grados respecto al eje vertical/horizontal estándar:
#   torso_lean: inclinación del torso (+ adelante según facing)
#   sh_f / el_f: hombro y codo del brazo delantero
#   sh_b / el_b: hombro y codo del brazo trasero
#   hip_f / knee_f: cadera y rodilla de la pierna delantera
#   hip_b / knee_b: cadera y rodilla de la pierna trasera
@dataclass(frozen=True)
class FighterPose:
    x: float
    y: float  # 0 = suelo del escenario, positivo = hacia arriba en el aire
    facing: float  # +1 mira derecha, -1 mira izquierda
    torso_lean: float
    head_tilt: float
    sh_f: float
    el_f: float
    sh_b: float
    el_b: float
    hip_f: float
    knee_f: float
    hip_b: float
    knee_b: float
    aura: float = 0.0
    charge_hand: float = 0.0


@dataclass(frozen=True)
class SoundCue:
    time_sec: float
    kind: str  # bell, dash, whoosh, light_hit, heavy_hit, parry, slam, charge, explosion, ko
    pan: float = 0.0  # -1 izq .. +1 der
    gain: float = 1.0


@dataclass(frozen=True)
class ImpactCue:
    time_sec: float
    x: float
    y: float
    color_bgr: Tuple[int, int, int]
    intensity: float  # 0.3 .. 1.6
    kind: str  # hit, parry, slam, explosion, ko


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def ease_in_out_cubic(t: float) -> float:
    t = max(0.0, min(1.0, t))
    if t < 0.5:
        return 4.0 * t * t * t
    return 1.0 - ((-2.0 * t + 2.0) ** 3) / 2.0


def ease_out_quad(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return 1.0 - (1.0 - t) * (1.0 - t)


def interpolate_pose(a: FighterPose, b: FighterPose, t: float, smooth: bool = True) -> FighterPose:
    u = ease_in_out_cubic(t) if smooth else max(0.0, min(1.0, t))
    return FighterPose(
        x=lerp(a.x, b.x, u),
        y=lerp(a.y, b.y, u),
        facing=1.0 if lerp(a.facing, b.facing, u) >= 0.0 else -1.0,
        torso_lean=lerp(a.torso_lean, b.torso_lean, u),
        head_tilt=lerp(a.head_tilt, b.head_tilt, u),
        sh_f=lerp(a.sh_f, b.sh_f, u),
        el_f=lerp(a.el_f, b.el_f, u),
        sh_b=lerp(a.sh_b, b.sh_b, u),
        el_b=lerp(a.el_b, b.el_b, u),
        hip_f=lerp(a.hip_f, b.hip_f, u),
        knee_f=lerp(a.knee_f, b.knee_f, u),
        hip_b=lerp(a.hip_b, b.hip_b, u),
        knee_b=lerp(a.knee_b, b.knee_b, u),
        aura=lerp(a.aura, b.aura, u),
        charge_hand=lerp(a.charge_hand, b.charge_hand, u),
    )


def pose_template(name: str, x: float, y: float, facing: float, aura: float = 0.0, charge: float = 0.0) -> FighterPose:
    """Biblioteca de poses marciales expresivas para combate stickman."""
    presets: Dict[str, Tuple[float, float, float, float, float, float, float, float, float, float]] = {
        # (torso_lean, head_tilt, sh_f, el_f, sh_b, el_b, hip_f, knee_f, hip_b, knee_b)
        "idle": (10, -4, 38, -88, -18, -95, 24, 20, -24, 18),
        "crouch": (28, -10, 50, -105, -25, -105, 58, 70, -40, 68),
        "dash": (38, -14, -45, -25, -60, -20, 48, 42, -55, 25),
        "jab": (24, -6, 86, -4, -32, -98, 34, 24, -36, 18),
        "cross": (32, -10, -35, -95, 92, -3, 38, 22, -40, 20),
        "uppercut": (-8, 12, 138, -32, -30, -88, 32, 34, -26, 20),
        "high_kick": (-24, 14, 30, -90, -52, -30, 108, -6, -18, 16),
        "slam_down": (44, -18, 125, -10, 115, -15, 36, 55, -35, 52),
        "guard": (-4, 8, 52, -118, 36, -122, 22, 34, -26, 26),
        "hit_back": (-28, 22, -38, -28, -62, -18, 18, 24, -30, 32),
        "air_flip": (-15, 18, -65, -45, 70, -50, 72, 85, -45, 78),
        "slide_kick": (-52, 24, -55, -20, -25, -65, 96, 6, -34, 72),
        "charge": (16, -8, -35, -75, -48, -65, 35, 40, -35, 34),
        "beam_fire": (22, -6, 90, 0, 84, -4, 36, 28, -38, 20),
        "ko_down": (-84, 20, -45, -18, -75, -15, 18, 20, -14, 28),
        "victory": (-6, -8, 148, -12, -25, -72, 20, 14, -22, 16),
    }
    p = presets[name]
    return FighterPose(
        x=x,
        y=y,
        facing=facing,
        torso_lean=p[0],
        head_tilt=p[1],
        sh_f=p[2],
        el_f=p[3],
        sh_b=p[4],
        el_b=p[5],
        hip_f=p[6],
        knee_f=p[7],
        hip_b=p[8],
        knee_b=p[9],
        aura=aura,
        charge_hand=charge,
    )


def _limb_vec(angle_deg: float, length: float, facing: float) -> Tuple[float, float]:
    """Convierte ángulo marcial (0=abajo, +90=adelante, +180=arriba) a delta pantalla (+X derecha, +Y abajo)."""
    rad = math.radians(angle_deg)
    dx = math.sin(rad) * length * facing
    dy = math.cos(rad) * length
    return (dx, dy)


def compute_skeleton_points(
    pose: FighterPose,
    ground_y: float,
    scale: float = 1.0,
) -> Dict[str, Tuple[float, float]]:
    """Calcula articulaciones exactas en coordenadas de mundo (Y hacia abajo)."""
    pelvis_x = pose.x
    pelvis_y = ground_y - pose.y - 74.0 * scale

    torso_rad = math.radians(pose.torso_lean)
    torso_len = 56.0 * scale
    neck_x = pelvis_x + math.sin(torso_rad) * torso_len * pose.facing
    neck_y = pelvis_y - math.cos(torso_rad) * torso_len

    head_rad = math.radians(pose.torso_lean + pose.head_tilt)
    head_x = neck_x + math.sin(head_rad) * (20.0 * scale) * pose.facing
    head_y = neck_y - math.cos(head_rad) * (20.0 * scale)

    shoulder_x = pelvis_x + math.sin(torso_rad) * (46.0 * scale) * pose.facing
    shoulder_y = pelvis_y - math.cos(torso_rad) * (46.0 * scale)

    upper_arm = 31.0 * scale
    forearm = 29.0 * scale
    thigh = 38.0 * scale
    shin = 38.0 * scale

    # Brazo delantero
    d_shf = _limb_vec(pose.sh_f, upper_arm, pose.facing)
    elbow_f = (shoulder_x + d_shf[0], shoulder_y + d_shf[1])
    d_elf = _limb_vec(pose.sh_f - pose.el_f, forearm, pose.facing)
    hand_f = (elbow_f[0] + d_elf[0], elbow_f[1] + d_elf[1])

    # Brazo trasero
    d_shb = _limb_vec(pose.sh_b, upper_arm, pose.facing)
    elbow_b = (shoulder_x + d_shb[0], shoulder_y + d_shb[1])
    d_elb = _limb_vec(pose.sh_b - pose.el_b, forearm, pose.facing)
    hand_b = (elbow_b[0] + d_elb[0], elbow_b[1] + d_elb[1])

    # Pierna delantera
    d_hf = _limb_vec(pose.hip_f, thigh, pose.facing)
    knee_f = (pelvis_x + d_hf[0], pelvis_y + d_hf[1])
    d_kf = _limb_vec(pose.hip_f - pose.knee_f, shin, pose.facing)
    foot_f = (knee_f[0] + d_kf[0], knee_f[1] + d_kf[1])

    # Pierna trasera
    d_hb = _limb_vec(pose.hip_b, thigh, pose.facing)
    knee_b = (pelvis_x + d_hb[0], pelvis_y + d_hb[1])
    d_kb = _limb_vec(pose.hip_b - pose.knee_b, shin, pose.facing)
    foot_b = (knee_b[0] + d_kb[0], knee_b[1] + d_kb[1])

    return {
        "pelvis": (pelvis_x, pelvis_y),
        "neck": (neck_x, neck_y),
        "head": (head_x, head_y),
        "shoulder": (shoulder_x, shoulder_y),
        "elbow_f": elbow_f,
        "hand_f": hand_f,
        "elbow_b": elbow_b,
        "hand_b": hand_b,
        "knee_f": knee_f,
        "foot_f": foot_f,
        "knee_b": knee_b,
        "foot_b": foot_b,
    }


def _sample_track(keyframes: List[Tuple[float, FighterPose]], t: float) -> FighterPose:
    if t <= keyframes[0][0]:
        return keyframes[0][1]
    for idx in range(len(keyframes) - 1):
        t0, p0 = keyframes[idx]
        t1, p1 = keyframes[idx + 1]
        if t <= t1:
            span = max(0.001, t1 - t0)
            return interpolate_pose(p0, p1, (t - t0) / span)
    return keyframes[-1][1]


def build_fight_choreography() -> Tuple[
    List[Tuple[float, FighterPose]],
    List[Tuple[float, FighterPose]],
    List[Tuple[float, float, float]],  # (t, blue_hp, red_hp)
    List[ImpactCue],
    List[SoundCue],
]:
    """
    Coreografía completa de 20.00 segundos (Stickman Azul vs Stickman Rojo):
      Acto 1 (0.0 - 1.8s): Entrada aérea y guardia ("¡PELEA!")
      Acto 2 (1.8 - 5.2s): Ráfaga de Azul, bloqueos de Rojo y uppercut aéreo
      Acto 3 (5.2 - 9.0s): Voltereta aérea de Azul, barrida y azote contra el suelo
      Acto 4 (9.0 - 13.0s): Contraataque veloz de Rojo + choque rápido de puños + patada alta
      Acto 5 (13.0 - 17.5s): Carga de energía de Azul + choque meteórico central
      Acto 6 (17.5 - 20.0s): Remate ascendente de Azul y K.O. definitivo sobre Rojo
    """
    blue_keys: List[Tuple[float, FighterPose]] = [
        (0.00, pose_template("air_flip", 310, 260, +1)),
        (0.55, pose_template("crouch", 340, 0, +1)),
        (0.95, pose_template("idle", 340, 0, +1)),
        (1.65, pose_template("idle", 355, 0, +1)),
        # Acto 2: Dash + combo sobre Rojo
        (1.90, pose_template("dash", 495, 8, +1)),
        (2.15, pose_template("jab", 565, 0, +1)),
        (2.40, pose_template("cross", 585, 0, +1)),
        (2.75, pose_template("high_kick", 605, 18, +1)),
        # Rojo bloquea y lanza uppercut a Azul
        (3.15, pose_template("crouch", 595, 0, +1)),
        (3.45, pose_template("hit_back", 525, 95, +1)),
        (4.10, pose_template("hit_back", 420, 195, +1)),
        (4.80, pose_template("air_flip", 360, 150, +1)),
        # Acto 3: Aterrizaje + barrida + slam
        (5.25, pose_template("crouch", 380, 0, +1)),
        (5.65, pose_template("dash", 530, 6, +1)),
        (6.05, pose_template("slide_kick", 670, 0, +1)),
        (6.75, pose_template("air_flip", 730, 185, +1)),
        (7.45, pose_template("slam_down", 755, 22, +1, aura=0.5)),
        (8.40, pose_template("idle", 690, 0, +1)),
        # Acto 4: Intercambio frenético ORA-style
        (9.20, pose_template("guard", 620, 0, +1)),
        (9.55, pose_template("jab", 620, 0, +1)),
        (9.80, pose_template("cross", 625, 0, +1)),
        (10.05, pose_template("jab", 625, 0, +1)),
        (10.30, pose_template("guard", 620, 0, +1)),
        (10.80, pose_template("hit_back", 530, 28, +1)),
        (11.40, pose_template("hit_back", 370, 88, +1)),
        (12.20, pose_template("crouch", 300, 0, +1)),
        (12.85, pose_template("idle", 315, 0, +1)),
        # Acto 5: Carga azul + choque central
        (13.20, pose_template("charge", 290, 0, +1, aura=0.85, charge=1.0)),
        (14.55, pose_template("charge", 295, 0, +1, aura=1.0, charge=1.0)),
        (14.95, pose_template("beam_fire", 420, 12, +1, aura=1.0, charge=1.0)),
        (15.70, pose_template("beam_fire", 575, 18, +1, aura=1.0, charge=0.9)),
        (16.95, pose_template("crouch", 590, 0, +1, aura=0.9)),
        # Acto 6: Finisher Uppercut + Victoria
        (17.45, pose_template("uppercut", 645, 38, +1, aura=1.0)),
        (18.25, pose_template("idle", 615, 0, +1, aura=0.4)),
        (19.00, pose_template("victory", 600, 0, +1, aura=0.35)),
        (20.00, pose_template("victory", 600, 0, +1, aura=0.25)),
    ]

    red_keys: List[Tuple[float, FighterPose]] = [
        (0.00, pose_template("air_flip", 970, 260, -1)),
        (0.55, pose_template("crouch", 940, 0, -1)),
        (0.95, pose_template("idle", 940, 0, -1)),
        (1.65, pose_template("idle", 925, 0, -1)),
        # Acto 2: Bloquea combo y contragolpea con uppercut
        (1.95, pose_template("guard", 720, 0, -1)),
        (2.15, pose_template("guard", 685, 0, -1)),
        (2.40, pose_template("guard", 695, 0, -1)),
        (2.75, pose_template("hit_back", 735, 10, -1)),
        (3.10, pose_template("crouch", 665, 0, -1)),
        (3.45, pose_template("uppercut", 625, 24, -1, aura=0.5)),
        (4.55, pose_template("idle", 650, 0, -1)),
        # Acto 3: Recibe barrida y slam
        (5.70, pose_template("idle", 760, 0, -1)),
        (6.10, pose_template("hit_back", 785, 65, -1)),
        (6.85, pose_template("hit_back", 815, 145, -1)),
        (7.50, pose_template("ko_down", 830, 0, -1)),
        (8.35, pose_template("crouch", 845, 0, -1, aura=0.6)),
        # Acto 4: Furioso contraataque + patada giratoria
        (8.95, pose_template("dash", 735, 12, -1, aura=0.9)),
        (9.55, pose_template("cross", 705, 0, -1, aura=0.9)),
        (9.80, pose_template("jab", 705, 0, -1, aura=0.9)),
        (10.05, pose_template("cross", 700, 0, -1, aura=0.9)),
        (10.35, pose_template("crouch", 680, 0, -1, aura=0.95)),
        (10.80, pose_template("high_kick", 615, 20, -1, aura=1.0)),
        (11.95, pose_template("idle", 720, 0, -1, aura=0.7)),
        (12.80, pose_template("charge", 955, 0, -1, aura=0.95, charge=0.8)),
        # Acto 5: Embestida meteórica y choque
        (14.55, pose_template("charge", 950, 0, -1, aura=1.0, charge=1.0)),
        (15.05, pose_template("dash", 770, 26, -1, aura=1.0, charge=1.0)),
        (15.70, pose_template("cross", 680, 18, -1, aura=1.0, charge=1.0)),
        (16.85, pose_template("hit_back", 730, 22, -1, aura=0.5)),
        # Acto 6: K.O.!
        (17.45, pose_template("hit_back", 735, 72, -1)),
        (18.10, pose_template("hit_back", 945, 165, -1)),
        (18.65, pose_template("ko_down", 1030, 0, -1)),
        (20.00, pose_template("ko_down", 1030, 0, -1)),
    ]

    hp_keys: List[Tuple[float, float, float]] = [
        (0.00, 1.00, 1.00),
        (2.70, 1.00, 0.92),
        (3.45, 0.78, 0.92),
        (6.10, 0.78, 0.76),
        (7.50, 0.78, 0.56),
        (10.80, 0.42, 0.50),
        (15.70, 0.24, 0.24),
        (17.45, 0.24, 0.00),
        (20.00, 0.24, 0.00),
    ]

    impacts: List[ImpactCue] = [
        ImpactCue(2.15, 625, 470, (255, 220, 80), 0.75, "parry"),
        ImpactCue(2.40, 635, 465, (255, 220, 80), 0.80, "parry"),
        ImpactCue(2.75, 670, 440, (255, 190, 60), 0.95, "hit"),
        ImpactCue(3.45, 585, 425, (70, 90, 255), 1.25, "hit"),
        ImpactCue(6.05, 735, 530, (255, 190, 60), 0.95, "hit"),
        ImpactCue(7.50, 795, 555, (255, 210, 90), 1.40, "slam"),
        ImpactCue(9.55, 665, 465, (255, 230, 110), 0.75, "parry"),
        ImpactCue(9.80, 665, 455, (255, 230, 110), 0.75, "parry"),
        ImpactCue(10.05, 665, 470, (255, 230, 110), 0.80, "parry"),
        ImpactCue(10.80, 575, 440, (70, 85, 255), 1.35, "hit"),
        ImpactCue(15.70, 640, 445, (255, 255, 255), 1.65, "explosion"),
        ImpactCue(17.45, 690, 415, (255, 220, 90), 1.75, "ko"),
        ImpactCue(18.65, 1020, 555, (180, 180, 200), 1.00, "slam"),
    ]

    sounds: List[SoundCue] = [
        SoundCue(0.54, "slam", -0.3, 0.55),
        SoundCue(0.56, "slam", +0.3, 0.55),
        SoundCue(1.25, "bell", 0.0, 0.95),
        SoundCue(1.85, "dash", -0.3, 0.85),
        SoundCue(2.12, "whoosh", -0.1, 0.75),
        SoundCue(2.15, "parry", 0.0, 0.95),
        SoundCue(2.38, "whoosh", 0.0, 0.75),
        SoundCue(2.40, "parry", 0.05, 0.95),
        SoundCue(2.72, "whoosh", 0.1, 0.85),
        SoundCue(2.75, "light_hit", 0.1, 0.95),
        SoundCue(3.40, "whoosh", 0.0, 0.95),
        SoundCue(3.45, "heavy_hit", -0.05, 1.10),
        SoundCue(4.78, "whoosh", -0.4, 0.75),
        SoundCue(5.25, "slam", -0.4, 0.50),
        SoundCue(5.65, "dash", -0.2, 0.85),
        SoundCue(6.05, "light_hit", 0.15, 0.95),
        SoundCue(6.75, "whoosh", 0.2, 0.80),
        SoundCue(7.50, "slam", 0.25, 1.15),
        SoundCue(8.92, "dash", 0.25, 0.90),
        SoundCue(9.55, "parry", 0.0, 0.85),
        SoundCue(9.80, "parry", 0.0, 0.85),
        SoundCue(10.05, "parry", 0.0, 0.90),
        SoundCue(10.74, "whoosh", -0.05, 0.95),
        SoundCue(10.80, "heavy_hit", -0.1, 1.15),
        SoundCue(13.20, "charge", -0.5, 0.95),
        SoundCue(14.95, "dash", 0.1, 0.95),
        SoundCue(15.70, "explosion", 0.0, 1.25),
        SoundCue(17.40, "whoosh", 0.05, 1.0),
        SoundCue(17.45, "ko", 0.1, 1.30),
        SoundCue(18.65, "slam", 0.55, 0.85),
    ]

    return blue_keys, red_keys, hp_keys, impacts, sounds


def sample_fight_state(
    t: float,
    blue_keys: List[Tuple[float, FighterPose]],
    red_keys: List[Tuple[float, FighterPose]],
    hp_keys: List[Tuple[float, float, float]],
) -> Tuple[FighterPose, FighterPose, float, float]:
    blue = _sample_track(blue_keys, t)
    red = _sample_track(red_keys, t)
    blue_hp = hp_keys[-1][1]
    red_hp = hp_keys[-1][2]
    for idx in range(len(hp_keys) - 1):
        t0, b0, r0 = hp_keys[idx]
        t1, b1, r1 = hp_keys[idx + 1]
        if t <= t1:
            u = ease_out_quad((t - t0) / max(0.001, t1 - t0))
            blue_hp = lerp(b0, b1, u)
            red_hp = lerp(r0, r1, u)
            break
    return blue, red, blue_hp, red_hp


def _env_decay(n: int, sr: int, attack_s: float, decay_s: float) -> np.ndarray:
    t = np.arange(n, dtype=np.float32) / float(sr)
    att = np.minimum(1.0, t / max(0.0005, attack_s))
    dec = np.exp(-t / max(0.005, decay_s))
    return (att * dec).astype(np.float32)


def synthesize_sound_cue(kind: str, sr: int = 48000, seed: int = 42) -> np.ndarray:
    """Genera un efecto de sonido nítido y con pegada mediante DSP puro en NumPy."""
    rng = np.random.default_rng(seed)

    if kind == "whoosh":
        dur = 0.24
        n = int(sr * dur)
        noise = rng.normal(0.0, 1.0, n).astype(np.float32)
        t = np.linspace(0.0, 1.0, n, dtype=np.float32)
        sweep = np.sin(2.0 * math.pi * (280.0 + 950.0 * t) * (t * dur))
        env = np.maximum(0.0, np.sin(math.pi * t)) ** 1.6
        return (0.42 * noise * env + 0.22 * sweep * env).astype(np.float32)

    if kind == "dash":
        dur = 0.28
        n = int(sr * dur)
        t = np.arange(n, dtype=np.float32) / sr
        freq = 160.0 + 1300.0 * (t / dur) ** 1.4
        phase = 2.0 * math.pi * np.cumsum(freq) / sr
        noise = rng.normal(0.0, 0.35, n).astype(np.float32)
        env = _env_decay(n, sr, 0.01, 0.08)
        return ((0.55 * np.sin(phase) + noise) * env).astype(np.float32)

    if kind == "light_hit":
        dur = 0.20
        n = int(sr * dur)
        t = np.arange(n, dtype=np.float32) / sr
        freq = 210.0 * np.exp(-t / 0.03) + 55.0
        body = np.sin(2.0 * math.pi * np.cumsum(freq) / sr) * _env_decay(n, sr, 0.002, 0.045)
        crack = rng.normal(0.0, 1.0, n).astype(np.float32) * _env_decay(n, sr, 0.001, 0.018)
        return np.tanh(0.95 * body + 0.75 * crack).astype(np.float32)

    if kind in ("heavy_hit", "slam"):
        dur = 0.45 if kind == "slam" else 0.34
        n = int(sr * dur)
        t = np.arange(n, dtype=np.float32) / sr
        f0 = 155.0 if kind == "slam" else 220.0
        freq = f0 * np.exp(-t / 0.05) + 42.0
        sub = np.sin(2.0 * math.pi * np.cumsum(freq) / sr) * _env_decay(n, sr, 0.002, 0.14)
        crunch = rng.normal(0.0, 1.0, n).astype(np.float32) * _env_decay(n, sr, 0.001, 0.055)
        return np.tanh(1.25 * sub + 0.85 * crunch).astype(np.float32)

    if kind == "parry":
        dur = 0.32
        n = int(sr * dur)
        t = np.arange(n, dtype=np.float32) / sr
        metal = (
            0.50 * np.sin(2.0 * math.pi * 1480.0 * t)
            + 0.35 * np.sin(2.0 * math.pi * 2310.0 * t)
            + 0.25 * np.sin(2.0 * math.pi * 3420.0 * t)
        )
        spark = rng.normal(0.0, 0.4, n).astype(np.float32) * _env_decay(n, sr, 0.001, 0.025)
        return (metal * _env_decay(n, sr, 0.001, 0.085) + spark).astype(np.float32)

    if kind == "charge":
        dur = 1.65
        n = int(sr * dur)
        t = np.arange(n, dtype=np.float32) / sr
        freq = 120.0 + 620.0 * (t / dur) ** 1.8
        phase = 2.0 * math.pi * np.cumsum(freq) / sr
        wobble = 0.65 + 0.35 * np.sin(2.0 * math.pi * 11.0 * t)
        env = np.minimum(1.0, t / 0.35) * np.exp(-np.maximum(0.0, t - 1.35) / 0.10)
        return (0.48 * np.sin(phase) * wobble * env + 0.16 * np.sin(2.0 * phase) * env).astype(np.float32)

    if kind in ("explosion", "ko"):
        dur = 1.15 if kind == "ko" else 0.90
        n = int(sr * dur)
        t = np.arange(n, dtype=np.float32) / sr
        freq = 160.0 * np.exp(-t / 0.12) + 34.0
        boom = np.sin(2.0 * math.pi * np.cumsum(freq) / sr) * _env_decay(n, sr, 0.003, 0.34)
        rumble = rng.normal(0.0, 1.0, n).astype(np.float32) * _env_decay(n, sr, 0.004, 0.28)
        gong = 0.0
        if kind == "ko":
            gong = (
                0.45 * np.sin(2.0 * math.pi * 523.25 * t)
                + 0.30 * np.sin(2.0 * math.pi * 783.99 * t)
                + 0.25 * np.sin(2.0 * math.pi * 1046.5 * t)
            ) * _env_decay(n, sr, 0.004, 0.45)
        return np.tanh(1.3 * boom + 0.9 * rumble + gong).astype(np.float32)

    # bell
    dur = 0.85
    n = int(sr * dur)
    t = np.arange(n, dtype=np.float32) / sr
    bell = (
        0.55 * np.sin(2.0 * math.pi * 880.0 * t)
        + 0.32 * np.sin(2.0 * math.pi * 1320.0 * t)
        + 0.20 * np.sin(2.0 * math.pi * 2200.0 * t)
    ) * _env_decay(n, sr, 0.002, 0.26)
    return bell.astype(np.float32)


def synthesize_fight_soundtrack(duration_sec: float, sounds: List[SoundCue], sr: int = 48000) -> np.ndarray:
    """
    Genera pista estéreo de 48 kHz combinando música electrónica de combate (138 BPM)
    con todos los efectos de sonido colocados en su milisegundo exacto.
    """
    total_samples = int(round(duration_sec * sr))
    mix = np.zeros((total_samples, 2), dtype=np.float32)
    rng = np.random.default_rng(7)

    # 1. Base musical rítmica 138 BPM (se atenúa dramáticamente tras el K.O. en 17.5s)
    bpm = 138.0
    step_sec = 60.0 / bpm / 4.0  # semicorchea
    bass_notes = [55.0, 55.0, 65.41, 73.42, 55.0, 55.0, 82.41, 73.42]
    lead_notes = [440.0, 523.25, 587.33, 659.25, 587.33, 523.25, 440.0, 392.0]

    step_idx = 0
    while True:
        t_step = step_idx * step_sec
        if t_step >= 17.65:
            break
        s0 = int(t_step * sr)
        bgm_gain = 1.0 if t_step < 17.35 else max(0.0, 1.0 - (t_step - 17.35) / 0.3)

        # Kick en tiempos 0, 4, 8, 12
        if step_idx % 4 == 0:
            kn = min(int(0.16 * sr), total_samples - s0)
            if kn > 0:
                tk = np.arange(kn, dtype=np.float32) / sr
                kf = 130.0 * np.exp(-tk / 0.032) + 46.0
                kick = np.sin(2.0 * math.pi * np.cumsum(kf) / sr) * _env_decay(kn, sr, 0.001, 0.07) * 0.42 * bgm_gain
                mix[s0 : s0 + kn, 0] += kick
                mix[s0 : s0 + kn, 1] += kick

        # Snare en 4 y 12
        if step_idx % 8 == 4:
            sn = min(int(0.14 * sr), total_samples - s0)
            if sn > 0:
                ts = np.arange(sn, dtype=np.float32) / sr
                snare = (
                    0.5 * np.sin(2.0 * math.pi * 195.0 * ts) * _env_decay(sn, sr, 0.001, 0.04)
                    + rng.normal(0.0, 0.55, sn).astype(np.float32) * _env_decay(sn, sr, 0.001, 0.06)
                ) * 0.28 * bgm_gain
                mix[s0 : s0 + sn, 0] += snare
                mix[s0 : s0 + sn, 1] += snare

        # Hi-hat en corcheas
        if step_idx % 2 == 1:
            hn = min(int(0.05 * sr), total_samples - s0)
            if hn > 0:
                hat = rng.normal(0.0, 0.25, hn).astype(np.float32) * _env_decay(hn, sr, 0.001, 0.015) * 0.20 * bgm_gain
                mix[s0 : s0 + hn, 0] += hat * 0.85
                mix[s0 : s0 + hn, 1] += hat * 1.15

        # Bajo pulsante en semicorcheas
        bn = min(int(step_sec * 0.92 * sr), total_samples - s0)
        if bn > 0:
            tb = np.arange(bn, dtype=np.float32) / sr
            bf = bass_notes[(step_idx // 2) % len(bass_notes)]
            bass = np.tanh(1.6 * np.sin(2.0 * math.pi * bf * tb)) * _env_decay(bn, sr, 0.004, 0.09) * 0.22 * bgm_gain
            mix[s0 : s0 + bn, 0] += bass
            mix[s0 : s0 + bn, 1] += bass

        # Lead synth de clímax a partir del segundo 8.5
        if t_step >= 8.5 and step_idx % 2 == 0:
            ln = min(int(step_sec * 1.6 * sr), total_samples - s0)
            if ln > 0:
                tl = np.arange(ln, dtype=np.float32) / sr
                lf = lead_notes[(step_idx // 2) % len(lead_notes)]
                lead = (
                    0.6 * np.sin(2.0 * math.pi * lf * tl) + 0.3 * np.sin(4.0 * math.pi * lf * tl)
                ) * _env_decay(ln, sr, 0.006, 0.14) * 0.14 * bgm_gain
                mix[s0 : s0 + ln, 0] += lead * 1.1
                mix[s0 : s0 + ln, 1] += lead * 0.9

        step_idx += 1

    # 2. Mezclar cada SFX en estéreo con paneo por posición en el escenario
    cache: Dict[str, np.ndarray] = {}
    for idx, cue in enumerate(sounds):
        if cue.kind not in cache:
            cache[cue.kind] = synthesize_sound_cue(cue.kind, sr=sr, seed=100 + idx)
        sfx = cache[cue.kind] * cue.gain
        start_idx = max(0, int(round(cue.time_sec * sr)))
        if start_idx >= total_samples:
            continue
        usable = min(len(sfx), total_samples - start_idx)
        pan = max(-1.0, min(1.0, cue.pan))
        gain_l = math.sqrt(0.5 * (1.0 - pan)) * 0.85
        gain_r = math.sqrt(0.5 * (1.0 + pan)) * 0.85
        mix[start_idx : start_idx + usable, 0] += sfx[:usable] * gain_l
        mix[start_idx : start_idx + usable, 1] += sfx[:usable] * gain_r

    # Limitador suave para evitar clipping y preservar pegada
    peak = float(np.max(np.abs(mix))) if total_samples > 0 else 1.0
    if peak > 0.92:
        mix = np.tanh(mix / max(1.0, peak * 0.92)) * 0.92
    return (np.clip(mix, -1.0, 1.0) * 32767.0).astype(np.int16)
