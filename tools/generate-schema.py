#!/usr/bin/env python3
"""Regenerate the editor JSON Schema; Rust validation remains authoritative."""
import json
from pathlib import Path
s = {"type": "string"}
nullable = {"type": ["string", "null"]}
b = {"type": "boolean", "default": False}
argv = {"type": "array", "items": s, "default": []}
env = {"type": "object", "propertyNames": {"pattern": "^(?!GSR_)[A-Za-z_][A-Za-z0-9_]*$"}, "additionalProperties": s}
def obj(props, required=()):
    result = {"type": "object", "additionalProperties": False, "properties": props}
    if required: result["required"] = list(required)
    return result
def ref(name): return {"$ref": "#/$defs/" + name}
def integer(default, maximum, minimum=0): return {"type": "integer", "minimum": minimum, "maximum": maximum, "default": default}
wrappers = {"type": "array", "items": {**argv, "minItems": 1}}
action = {"name": nullable, "command": argv, "script": nullable, "args": argv, "env": env,
          "working_dir": nullable, "scope": {"enum": ["host", "game"], "default": "game"}}
choice = [{"required": ["command"], "properties": {"command": {**argv, "minItems": 1}, "script": {"type": "null"}, "args": {**argv, "maxItems": 0}}},
          {"required": ["script"], "properties": {"script": {"type": "string", "minLength": 1}, "command": {**argv, "maxItems": 0}}}]
defs = {
    "overlays": obj({"gamemode": b, "mangohud": b, "mangohud_config": nullable, "gamescope": {"type": ["array", "null"], "items": s}, "vkbasalt": b, "vkbasalt_config": nullable, "dxvk_hud": nullable}),
    "proton": {"anyOf": [{"type": "null"}, obj({"path": nullable, "prefix": nullable, "steam_root": nullable, "verb": {"enum": [None, "run", "waitforexitandrun", "runinprefix"]}, "env": env})]},
    "runtime": obj({"mode": {"enum": ["auto", "required", "none"], "default": "auto"}, "command": argv}),
    "script": {**obj({"body": nullable, "path": nullable}), "oneOf": [
        {"required": ["body"], "properties": {"body": {"type": "string", "minLength": 1}, "path": {"type": "null"}}},
        {"required": ["path"], "properties": {"path": {"type": "string", "minLength": 1}, "body": {"type": "null"}}}]},
    "action": {**obj({**action, "allow_failure": b, "timeout_ms": {"type": ["integer", "null"], "minimum": 1, "maximum": 86400000}}), "oneOf": choice},
    "companion": {**obj({**action, "name": {"type": "string", "minLength": 1}, "proton": b, "required": b,
        "start_delay_ms": integer(0, 3600000), "start_after_game": b, "ready_file": nullable,
        "ready_timeout_ms": integer(10000, 3600000, 1)}, ["name"]), "oneOf": choice},
}
schema = {"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "Game Super Runtime v1",
    "description": "Editor assistance; run game-super-runtime APP-ID --validate for semantic checks.",
    **obj({"version": {"const": 1, "default": 1}, "name": nullable, "command": argv,
    "working_dir": nullable, "variables": env, "env": env, "unset_env": argv,
    "wrappers": wrappers, "game_wrappers": wrappers, "overlays": ref("overlays"), "proton": ref("proton"),
    "runtime": ref("runtime"), "scripts": {"type": "object", "additionalProperties": ref("script")},
    "before": {"type": "array", "items": ref("action")}, "after": {"type": "array", "items": ref("action")},
    "companions": {"type": "array", "items": ref("companion")}, "shutdown_timeout_ms": integer(3000, 60000)}), "$defs": defs}
Path(__file__).resolve().parent.parent.joinpath("docs/config.schema.json").write_text(json.dumps(schema, indent=2) + "\n")
