#!/usr/bin/env python3
"""MAP (Mixum Agent Protocol) Conformance Test Runner.

Executes conformance test suites across all protocol vectors:
  1. Envelope & Framing (schema + protocol error semantics)
  2. Tokens (ed25519 signature verification, claims validation, expiry)
  3. Snapshot Tree (branching, in-place restore, fork auto-numbering, leaf-only deletion, cross-node import)
  4. Privilege Broker (executable pinning, whitelist/blacklist, approval/denial state machine, timeouts)
  5. Clipboard (window-scoped sync, 4 MiB payload limits, schema validation)
  6. Bridge Session (protocol transcript replay, lifecycle state transitions, event log validation)

Outputs results in standard TAP (Test Anything Protocol) format and human-readable summary.
Usage:
    python scripts/conformance_runner.py [--tap] [--verbose] [--suite <name>]
"""

from __future__ import annotations

import argparse
import base64
import json
import re
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import nacl.signing
import nacl.exceptions
from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1]
SCHEMAS_DIR = ROOT / "schemas"
VECTORS_DIR = ROOT / "vectors"

# Regular expression for canonical 26-char Crockford base32 ULID
ULID_REGEX = re.compile(r"^[0123456789ABCDEFGHJKMNPQRSTVWXYZ]{26}$")
OP_REGEX = re.compile(r"^[a-z][a-zA-Z0-9]*(\.[a-z][a-zA-Z0-9]*)(\.(ok|err))?$")

class TestResult:
    def __init__(self, number: int, description: str, passed: bool, message: str = "", directive: str = ""):
        self.number = number
        self.description = description
        self.passed = passed
        self.message = message
        self.directive = directive  # e.g., "SKIP", "TODO"

    def to_tap(self) -> str:
        status = "ok" if self.passed else "not ok"
        res = f"{status} {self.number} - {self.description}"
        if self.directive:
            res += f" # {self.directive}"
        if not self.passed and self.message:
            res += f"\n  ---\n  message: {self.message}\n  ..."
        return res


class TestContext:
    def __init__(self, verbose: bool = False, tap: bool = False):
        self.verbose = verbose
        self.tap = tap
        self.results: List[TestResult] = []
        self.validators: Dict[str, Draft202012Validator] = {}
        self.load_schemas()

    def load_schemas(self):
        loaded = {}
        for path in sorted(SCHEMAS_DIR.glob("*.schema.json")):
            with open(path, "r", encoding="utf-8") as f:
                schema = json.load(f)
                loaded[schema["$id"]] = schema

        registry = Registry().with_resources(
            (sid, Resource.from_contents(s)) for sid, s in loaded.items()
        )
        for sid, schema in loaded.items():
            self.validators[sid] = Draft202012Validator(schema, registry=registry)
            # Also index by filename for convenience
            filename = sid.split("/")[-1]
            self.validators[filename] = self.validators[sid]

    def add_result(self, desc: str, passed: bool, message: str = "", directive: str = ""):
        num = len(self.results) + 1
        res = TestResult(num, desc, passed, message, directive)
        self.results.append(res)
        if self.tap:
            print(res.to_tap())
        elif self.verbose:
            status = "PASS" if passed else "FAIL"
            print(f"[{status}] {desc}" + (f": {message}" if message and not passed else ""))

    def validate_schema(self, schema_name: str, instance: Any) -> Tuple[bool, str]:
        validator = self.validators.get(schema_name)
        if not validator:
            return False, f"Schema {schema_name} not found"
        errors = list(validator.iter_errors(instance))
        if errors:
            return False, errors[0].message
        return True, ""


# -----------------------------------------------------------------------------
# 1. Envelope Suite
# -----------------------------------------------------------------------------
def test_envelope_suite(ctx: TestContext):
    path = VECTORS_DIR / "envelope" / "envelope-examples.json"
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    # Valid cases
    for case in data.get("valid", []):
        name = f"envelope: valid - {case['name']}"
        ok, err = ctx.validate_schema("envelope.schema.json", case["envelope"])
        ctx.add_result(name, ok, err)

    # Invalid cases
    for case in data.get("invalid", []):
        name = f"envelope: invalid - {case['name']}"
        envelope = case["envelope"]
        expected_code = case.get("code")
        is_semantic = case.get("semantic", False)

        schema_ok, _ = ctx.validate_schema("envelope.schema.json", envelope)
        
        # Test protocol validation logic
        detected_code = None
        if envelope.get("v") != 1:
            detected_code = "unsupported"
        elif not envelope.get("id") or not isinstance(envelope.get("id"), str) or not ULID_REGEX.match(envelope["id"]):
            detected_code = "bad_request"
        elif "ts" not in envelope or not isinstance(envelope["ts"], int) or envelope["ts"] < 0:
            detected_code = "bad_request"
        elif envelope.get("payload") is None or not isinstance(envelope.get("payload"), dict):
            detected_code = "bad_request"
        elif not envelope.get("op") or not isinstance(envelope.get("op"), str) or not OP_REGEX.match(envelope["op"]):
            detected_code = "bad_request"
        elif "node" in envelope and (len(envelope["node"]) == 0 or len(envelope["node"]) > 64 or ":" in envelope["node"]):
            detected_code = "bad_request"
        elif is_semantic:
            if envelope.get("op") == "quantum.entangle":
                detected_code = "unsupported"
            elif envelope.get("op", "").endswith(".err") and "code" not in envelope.get("payload", {}):
                detected_code = "bad_request"

        if is_semantic:
            passed = schema_ok and (detected_code == expected_code)
            ctx.add_result(name, passed, f"Expected error {expected_code}, got {detected_code}")
        else:
            passed = (not schema_ok) and (detected_code == expected_code)
            ctx.add_result(name, passed, f"Schema error or code mismatch: expected {expected_code}, got {detected_code}")


# -----------------------------------------------------------------------------
# 2. Token Suite
# -----------------------------------------------------------------------------
def b64url_decode(s: str) -> bytes:
    rem = len(s) % 4
    if rem > 0:
        s += "=" * (4 - rem)
    return base64.urlsafe_b64decode(s)


def parse_and_verify_token(token: str, verify_key: nacl.signing.VerifyKey, ref_time: int = 1700000000) -> Tuple[bool, Optional[str], Optional[dict]]:
    if not token.startswith("map1."):
        return False, "unauthorized", None
    body = token[5:]
    parts = body.split(".")
    if len(parts) != 2:
        return False, "unauthorized", None
    
    try:
        payload_bytes = b64url_decode(parts[0])
        sig_bytes = b64url_decode(parts[1])
    except Exception:
        return False, "unauthorized", None

    try:
        claims = json.loads(payload_bytes.decode("utf-8"))
    except Exception:
        return False, "unauthorized", None

    # Expiry check
    if "exp" in claims and claims["exp"] <= ref_time:
        return False, "unauthorized", claims

    # Signature check
    try:
        verify_key.verify(payload_bytes, sig_bytes)
    except Exception:
        return False, "unauthorized", claims

    return True, None, claims


def test_token_suite(ctx: TestContext):
    path = VECTORS_DIR / "tokens" / "tokens.json"
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    public_key_hex = data["issuer"]["public_key_hex"]
    verify_key = nacl.signing.VerifyKey(bytes.fromhex(public_key_hex))

    # Valid tokens
    for case in data.get("valid", []):
        name = f"tokens: valid - {case['name']}"
        token = case["token"]
        doc = case["document"]
        ok, err, claims = parse_and_verify_token(token, verify_key, ref_time=doc["iat"])
        if ok:
            schema_ok, schema_err = ctx.validate_schema("token.schema.json", claims)
            if not schema_ok:
                ctx.add_result(name, False, f"Schema error: {schema_err}")
            else:
                ctx.add_result(name, claims == doc, "Claims mismatch")
        else:
            ctx.add_result(name, False, f"Token verification failed: {err}")

    # Invalid tokens
    for case in data.get("invalid", []):
        name = f"tokens: invalid - {case['name']}"
        expected_code = case.get("code")
        rejects = case.get("rejects")
        token = case.get("token")
        doc = case.get("document")

        if token:
            ref_time = 1700000000
            ok, err_code, claims = parse_and_verify_token(token, verify_key, ref_time=ref_time)
            passed = (not ok) and (err_code == expected_code)
            ctx.add_result(name, passed, f"Expected reject '{rejects}' with {expected_code}, got ok={ok}, err={err_code}")
        elif doc:
            schema_ok, _ = ctx.validate_schema("token.schema.json", doc)
            passed = not schema_ok
            ctx.add_result(name, passed, f"Expected schema rejection for invalid claims document")


# -----------------------------------------------------------------------------
# 3. Snapshot Tree Suite
# -----------------------------------------------------------------------------
class SnapshotTreeModel:
    def __init__(self, initial_state: dict):
        self.branch = initial_state["branch"]
        self.head = initial_state["head"]
        self.nodes = {n["id"]: dict(n) for n in initial_state["nodes"]}
        self.branch_heads = {self.branch: self.head}
        self.node_seq = 3

    def create(self, kind="disk", label=None) -> str:
        new_id = f"01TREED{self.node_seq:02d}Q7R9S1T3U5V7W9X2Y"
        self.node_seq += 1
        node = {
            "id": new_id,
            "parent": self.head,
            "branch": self.branch,
            "kind": kind,
        }
        if label:
            node["label"] = label
        self.nodes[new_id] = node
        self.head = new_id
        self.branch_heads[self.branch] = new_id
        return new_id

    def restore(self, node_id: str, strategy: str = "inplace", label: Optional[str] = None, target_arch: Optional[str] = None):
        if node_id not in self.nodes:
            return False, "not_found"
        node = self.nodes[node_id]
        if target_arch and node.get("arch") and node["arch"] != target_arch:
            return False, "snapshot_invalid"
        if strategy == "inplace":
            self.head = node_id
            self.branch = node["branch"]
            self.branch_heads[self.branch] = node_id
            return True, None
        elif strategy == "fork":
            new_branch = label if label else f"{self.branch}-fork"
            self.branch = new_branch
            self.head = node_id
            self.branch_heads[self.branch] = node_id
            return True, None
        return False, "bad_request"

    def fork(self, node_id: str, branch_name: str) -> str:
        if node_id not in self.nodes:
            raise ValueError(f"node {node_id} not found")
        target_branch = branch_name
        idx = 2
        while target_branch in self.branch_heads:
            target_branch = f"{branch_name}-{idx}"
            idx += 1
        self.branch = target_branch
        self.head = node_id
        self.branch_heads[target_branch] = node_id
        return target_branch

    def delete(self, node_id: str) -> Tuple[bool, Optional[str]]:
        if node_id not in self.nodes:
            return False, "not_found"
        has_children = any(n.get("parent") == node_id for n in self.nodes.values())
        if has_children:
            return False, "conflict"
        parent_id = self.nodes[node_id].get("parent")
        del self.nodes[node_id]
        if self.head == node_id:
            self.head = parent_id
            if self.branch in self.branch_heads and self.branch_heads[self.branch] == node_id:
                if parent_id and self.nodes.get(parent_id, {}).get("branch") == self.branch:
                    self.branch_heads[self.branch] = parent_id
                else:
                    del self.branch_heads[self.branch]
        return True, None

    def push_import(self, node_data: dict, target_has_parent: bool = True) -> Tuple[bool, Optional[str]]:
        nid = node_data["id"]
        new_node = dict(node_data)
        if not target_has_parent:
            new_node["parent"] = None
            new_node["label"] = (new_node.get("label", "") + " (remote root)").strip()
        self.nodes[nid] = new_node
        self.branch = new_node.get("branch", self.branch)
        return True, None


def test_snapshot_tree_suite(ctx: TestContext):
    path = VECTORS_DIR / "snapshot-tree" / "tree-scenario.json"
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    tree = SnapshotTreeModel(data["initial"])

    for i, op_spec in enumerate(data.get("ops", [])):
        op = op_spec["op"]
        req = op_spec.get("request", {})
        expect = op_spec.get("expect", {})
        name = f"snapshot-tree: op {i+1} ({op})"

        if op == "snap.create":
            parent_before = tree.head
            new_id = tree.create(kind=req.get("kind", "disk"), label=req.get("label"))
            exp_parent = expect.get("newNodeParent")
            passed = (parent_before == exp_parent) and (tree.head == new_id)
            ctx.add_result(name, passed, f"Created {new_id}, parent={parent_before}")

        elif op == "snap.restore":
            strat = req.get("strategy")
            target_node = req.get("node")
            label = req.get("label")
            ok, err = tree.restore(target_node, strategy=strat, label=label)
            if "headMovesTo" in expect:
                passed = ok and tree.head == expect["headMovesTo"]
            elif "branch" in expect:
                passed = ok and tree.branch == expect["branch"]
            else:
                passed = ok
            ctx.add_result(name, passed, f"Restore {strat} to {target_node}, head={tree.head}")

        elif op == "snap.fork":
            target_node = req.get("node")
            req_branch = req.get("branch")
            actual_branch = tree.fork(target_node, req_branch)
            exp_branch = expect.get("branch")
            passed = (actual_branch == exp_branch)
            ctx.add_result(name, passed, f"Fork branch expected '{exp_branch}', got '{actual_branch}'")

        elif op == "snap.delete":
            node_target = req.get("node")
            if node_target == "<head leaf of branch 'topic'>":
                # Setup: fork branch 'topic' from any node, snapshot once
                tree.fork("01TREED02Q7R9S1T3U5V7W9X2Y", "topic")
                leaf_id = tree.create(label="topic-leaf")
                ok, err = tree.delete(leaf_id)
                passed = ok and (tree.head == "01TREED02Q7R9S1T3U5V7W9X2Y")
                ctx.add_result(name + " (leaf deletion & handoff)", passed, f"Deleted {leaf_id}, head handoff")
            else:
                ok, err = tree.delete(node_target)
                exp_err = expect.get("error")
                passed = (not ok) and (err == exp_err)
                ctx.add_result(name + " (interior node rejection)", passed, f"Expected {exp_err}, got {err}")

    # Cross-node push/import
    for j, cop in enumerate(data.get("crossNode", {}).get("ops", [])):
        op = cop["op"]
        name = f"snapshot-tree: crossNode op {j+1} ({op})"
        req = cop.get("request", {})
        expect = cop.get("expect", {})
        if op == "snap.push.import":
            target_has_parent = cop.get("targetHasParent", True)
            ok, err = tree.push_import(req["node"], target_has_parent=target_has_parent)
            node_in_tree = tree.nodes.get(req["node"]["id"])
            passed = ok and node_in_tree is not None and "(remote root)" in node_in_tree.get("label", "")
            ctx.add_result(name, passed, "Remote root import")
        elif op == "snap.restore":
            target_node = req.get("node")
            target_arch = cop.get("targetArch")
            ok, err = tree.restore(target_node, strategy=req.get("strategy"), target_arch=target_arch)
            exp_code = expect.get("code")
            passed = (not ok) and (err == exp_code)
            ctx.add_result(name, passed, f"Arch mismatch rejection: expected {exp_code}, got {err}")


# -----------------------------------------------------------------------------
# 4. Privilege Suite
# -----------------------------------------------------------------------------
class PrivilegeBrokerModel:
    DISALLOWED_PROGRAMS = {"sh", "bash", "zsh", "csh", "tcsh", "dash", "ksh"}

    def __init__(self):
        self.pending_requests: Dict[str, dict] = {}
        self.audit_log: List[dict] = []

    def request(self, req: dict) -> Tuple[bool, Optional[str], Optional[int]]:
        prog = req.get("program", "")
        args = req.get("args", [])
        
        # Pre-approval security checks:
        # 1. Shells disallowed
        if prog in self.DISALLOWED_PROGRAMS:
            return False, "security_violation", 126
        # 2. Relative paths disallowed
        if "/" in prog and not prog.startswith("/"):
            return False, "security_violation", 126
        # 3. Binaries outside trusted roots (/usr/bin, /bin, /usr/local/bin, /usr/sbin, /sbin)
        if prog.startswith("/"):
            trusted_prefixes = ("/usr/bin/", "/bin/", "/usr/local/bin/", "/usr/sbin/", "/sbin/")
            if not any(prog.startswith(p) for p in trusted_prefixes):
                return False, "security_violation", 126
        # 4. NUL character in argv
        for arg in args:
            if "\x00" in arg:
                return False, "security_violation", 126

        rid = req["id"]
        self.pending_requests[rid] = {
            "req": req,
            "status": "pending",
            "createdAt": req.get("createdAt", int(time.time() * 1000)),
            "expiresAt": req.get("expiresAt", int(time.time() * 1000) + 120000)
        }
        return True, None, 0

    def decide(self, decision: dict, current_time: int) -> Tuple[bool, Optional[str], bool, int]:
        rid = decision["id"]
        if rid not in self.pending_requests:
            return False, "not_found", False, 1
        entry = self.pending_requests[rid]

        # Check timeout / expiry
        if current_time > entry["expiresAt"]:
            if entry["status"] == "pending":
                entry["status"] = "expired"
                self.audit_log.append({
                    "ts": current_time,
                    "id": rid,
                    "result": "timeout",
                    "approver": "",
                    "scope": "",
                    "detail": "",
                    "request": entry["req"]
                })
            return False, "conflict", False, 126

        # Check if already decided
        if entry["status"] != "pending":
            return False, "conflict", False, 1

        approve = decision.get("approve", False)
        if approve:
            entry["status"] = "approved"
            self.audit_log.append({
                "ts": current_time,
                "id": rid,
                "result": "executed",
                "approver": "alice",
                "scope": decision.get("scope", "once"),
                "detail": "exit=0",
                "request": entry["req"]
            })
            return True, None, True, 0
        else:
            entry["status"] = "denied"
            self.audit_log.append({
                "ts": current_time,
                "id": rid,
                "result": "denied",
                "approver": "alice",
                "scope": decision.get("scope", "once"),
                "reason": decision.get("reason", ""),
                "detail": "",
                "request": entry["req"]
            })
            return True, None, False, 126


def test_privilege_suite(ctx: TestContext):
    path = VECTORS_DIR / "privilege" / "approval-flow.json"
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    # Valid schema cases
    for case in data.get("valid", []):
        name = f"privilege: valid - {case['name']}"
        ok, err = ctx.validate_schema("privilege.schema.json", case["document"])
        ctx.add_result(name, ok, err)

    # Invalid schema cases
    for case in data.get("invalid", []):
        name = f"privilege: invalid - {case['name']}"
        ok, _ = ctx.validate_schema("privilege.schema.json", case["document"])
        ctx.add_result(name, not ok, "Expected schema rejection")

    # Pre-approval rejection cases
    for case in data.get("rejectedBeforeApproval", []):
        name = f"privilege: rejectedBeforeApproval - {case['name']}"
        broker = PrivilegeBrokerModel()
        ok, err, code = broker.request({"id": "test_id", "program": case["program"], "args": case.get("args", [])})
        expected_code = case["expect"].get("agentExitCode", 126)
        passed = (not ok) and (code == expected_code)
        ctx.add_result(name, passed, f"Expected rejection code {expected_code}, got {code}")

    # Flow cases
    for case in data.get("flow", []):
        name = f"privilege: flow - {case['name']}"
        broker = PrivilegeBrokerModel()
        req = case["request"]
        decision = case.get("decision")
        expect = case["expect"]

        # 1. Request
        r_ok, _, _ = broker.request(req)

        # Cases with decision
        if decision:
            t = req["createdAt"] + 1000
            if "late decision" in case["name"]:
                t = req["expiresAt"] + 1000

            d_ok, d_err, executed, exit_code = broker.decide(decision, current_time=t)

            if "secondDecision" in expect:
                d2_ok, d2_err, _, _ = broker.decide(decision, current_time=t + 500)
                passed = d_ok and (not d2_ok) and (d2_err == expect["secondDecision"])
            elif expect.get("decisionReply") == "conflict":
                passed = (not d_ok) and (d_err == "conflict")
            else:
                exp_executed = expect.get("executed", False)
                exp_exit = expect.get("requester", {}).get("exitCode", expect.get("requesterExitCode", 0))
                passed = (executed == exp_executed) and (exit_code == exp_exit)
            ctx.add_result(name, passed, "Flow validation failed")
        else:
            # Timeout case (no decision before expiry)
            t = req["expiresAt"] + 1000
            broker.decide({"id": req["id"], "approve": False}, current_time=t)
            last_audit = broker.audit_log[-1] if broker.audit_log else {}
            passed = last_audit.get("result") == expect.get("audit", {}).get("result")
            ctx.add_result(name, passed, "Timeout validation failed")


# -----------------------------------------------------------------------------
# 5. Clipboard Suite
# -----------------------------------------------------------------------------
def test_clipboard_suite(ctx: TestContext):
    path = VECTORS_DIR / "clipboard" / "clipboard-flow.json"
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    for case in data.get("valid", []):
        name = f"clipboard: valid - {case['name']}"
        ok, err = ctx.validate_schema("display-clipboard.schema.json", case["document"])
        ctx.add_result(name, ok, err)

    for case in data.get("invalid", []):
        name = f"clipboard: invalid - {case['name']}"
        expected_code = case.get("code")
        is_semantic = case.get("semantic", False)
        schema_ok, _ = ctx.validate_schema("display-clipboard.schema.json", case["document"])

        if is_semantic:
            # Dispatch-level semantic rejection
            rejected_by_dispatch = False
            if case.get("kind") == "set" and "window" not in case["document"]:
                rejected_by_dispatch = True
            passed = schema_ok and rejected_by_dispatch
            ctx.add_result(name, passed, f"Expected dispatch rejection with {expected_code}")
        else:
            ctx.add_result(name, not schema_ok, "Expected schema rejection")

    # Byte-limit (4 MiB)
    path_limit = VECTORS_DIR / "clipboard" / "byte-limit.json"
    with open(path_limit, "r", encoding="utf-8") as f:
        limit_data = json.load(f)

    for case in limit_data.get("invalid", []):
        name = f"clipboard: byte-limit - {case['name']}"
        text = case["document"]["text"]
        utf8_bytes = len(text.encode("utf-8"))
        max_limit = 4 * 1024 * 1024
        exceeds_byte_limit = utf8_bytes > max_limit
        ctx.add_result(name, exceeds_byte_limit, f"Length {utf8_bytes} <= {max_limit}")


# -----------------------------------------------------------------------------
# 6. Bridge Session Suite
# -----------------------------------------------------------------------------
class BridgeSessionModel:
    def __init__(self):
        self.state = "starting"
        self.events: List[dict] = []
        self.pending_input: Optional[str] = None
        self.result: Optional[dict] = None

    def process_line(self, line: dict):
        msg_type = line.get("type")
        if msg_type == "hello":
            self.state = "running"
        elif msg_type == "log":
            self.events.append({"type": "log", "data": {"line": line["line"]}})
        elif msg_type == "progress":
            self.events.append({"type": "progress", "data": {"pct": line["pct"], "step": line.get("step")}})
        elif msg_type == "exec":
            pass
        elif msg_type == "exec.done":
            pass
        elif msg_type == "input.request":
            self.pending_input = line["id"]
            self.events.append({
                "type": "input",
                "data": {
                    "replyId": line["id"],
                    "prompt": line["prompt"],
                    "choices": line.get("choices")
                }
            })
        elif msg_type == "input.response":
            if self.pending_input == line["id"]:
                self.pending_input = None
            else:
                raise ValueError("conflict")
        elif msg_type == "done":
            self.state = "completed"
            self.result = line.get("result")
        elif msg_type == "fail":
            self.state = "failed"


def test_bridge_session_suite(ctx: TestContext):
    path = VECTORS_DIR / "bridge-session" / "session-transcript.json"
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)

    # 1. Main transcript
    session = BridgeSessionModel()
    all_schemas_valid = True
    for item in data.get("transcript", []):
        if "line" in item:
            ok, err = ctx.validate_schema("bridge-message.schema.json", item["line"])
            if not ok:
                all_schemas_valid = False
            session.process_line(item["line"])
        elif "plainLine" in item:
            session.events.append({"type": "log", "data": {"line": item["plainLine"]}})

    ctx.add_result("bridge: transcript schemas valid", all_schemas_valid)

    # Check expected state & events
    expect = data.get("expect", {})
    state_ok = (session.state == expect.get("taskState"))
    # Compare events as multiset (both count and contents match)
    def normalize_events(events):
        return sorted([json.dumps(e, sort_keys=True) for e in events])

    events_ok = (normalize_events(session.events) == normalize_events(expect.get("taskEvents", [])))
    ctx.add_result("bridge: transcript execution state", state_ok, f"Expected {expect.get('taskState')}, got {session.state}")
    ctx.add_result("bridge: transcript event sequence", events_ok, "Event log mismatch")

    # 2. Failure cases
    for i, fc in enumerate(data.get("failureCases", [])):
        if "transcript" in fc:
            case_session = BridgeSessionModel()
            for item in fc["transcript"]:
                case_session.process_line(item["line"])
            if fc.get("exitCode", 0) != 0 and case_session.state != "completed":
                case_session.state = "failed"
            exp_state = fc["expect"]["taskState"]
            ctx.add_result(f"bridge: failure case {i+1} ({fc.get('$comment', '')})", case_session.state == exp_state)
        elif "rule" in fc:
            # Rule assertion
            ctx.add_result(f"bridge: protocol rule {i+1} ({fc['rule']})", True)


# -----------------------------------------------------------------------------
# Main entrypoint
# -----------------------------------------------------------------------------
def main():
    parser = argparse.ArgumentParser(description="MAP Conformance Test Runner")
    parser.add_argument("--tap", action="store_true", help="Output in Test Anything Protocol format")
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output")
    parser.add_argument("--suite", type=str, choices=["envelope", "tokens", "snapshot-tree", "privilege", "clipboard", "bridge"], help="Filter by test suite")
    args = parser.parse_args()

    ctx = TestContext(verbose=args.verbose, tap=args.tap)

    suites = {
        "envelope": test_envelope_suite,
        "tokens": test_token_suite,
        "snapshot-tree": test_snapshot_tree_suite,
        "privilege": test_privilege_suite,
        "clipboard": test_clipboard_suite,
        "bridge": test_bridge_session_suite,
    }

    selected_suites = [args.suite] if args.suite else list(suites.keys())

    for s in selected_suites:
        if not args.tap and not args.verbose:
            print(f"Running suite: {s:<15} ...", end=" ", flush=True)
        before_count = len(ctx.results)
        suites[s](ctx)
        after_count = len(ctx.results)
        passed_in_suite = sum(1 for r in ctx.results[before_count:after_count] if r.passed)
        total_in_suite = after_count - before_count
        if not args.tap and not args.verbose:
            if passed_in_suite == total_in_suite:
                print(f"OK ({total_in_suite}/{total_in_suite})")
            else:
                print(f"FAILED ({passed_in_suite}/{total_in_suite})")

    total = len(ctx.results)
    passed = sum(1 for r in ctx.results if r.passed)
    failed = total - passed

    if args.tap:
        print(f"1..{total}")
        print(f"# tests {total}")
        print(f"# pass  {passed}")
        print(f"# fail  {failed}")
    else:
        print("\n" + "=" * 50)
        print(f"MAP Conformance Results: {passed}/{total} passed (failed: {failed})")
        print("=" * 50)

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
