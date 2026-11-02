#!/usr/bin/env python3
"""Exercise Designer JSON-RPC source edits and native Studio hot reload under Wayland."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent


def wait_for(predicate, description, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.05)
    raise AssertionError(f"timed out: {description}")


def rpc_call(process, request_id, method, params=None):
    request = {"jsonrpc": "2.0", "id": request_id, "method": method}
    if params is not None:
        request["params"] = params
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    response = json.loads(process.stdout.readline())
    assert response.get("id") == request_id, response
    assert "error" not in response, response
    return response["result"]


def tool_call(process, request_id, name, arguments):
    return rpc_call(
        process,
        request_id,
        "tools/call",
        {"name": name, "arguments": arguments},
    )


def agent_write(process, request_id, path, content):
    read = tool_call(process, request_id, "project.read_file", {"path": path})
    assert not read["isError"], read
    revision = read["structuredContent"]["revision"]
    write = tool_call(
        process,
        request_id + 1,
        "project.write_file",
        {"path": path, "content": content, "expected_revision": revision},
    )
    assert not write["isError"], write
    return write["structuredContent"]["revision"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--studio", default=os.environ.get("STUDIO_CLI_BINARY", str(ROOT / "target/debug/studio")))
    parser.add_argument("--designer", default=os.environ.get("STUDIO_DESIGNER_BINARY", str(ROOT / "target/debug/studio-designer")))
    args = parser.parse_args()
    studio = str(Path(args.studio).resolve())
    designer = str(Path(args.designer).resolve())
    if not Path(studio).is_file():
        parser.error("build studio-cli first and pass --studio <binary>")
    if not Path(designer).is_file():
        parser.error("build studio-designer first and pass --designer <binary>")
    for command in ("sway", "swaymsg", "dbus-run-session"):
        if not shutil.which(command):
            parser.error(f"{command} is required")
    with tempfile.TemporaryDirectory(prefix="studio-hmr-") as temporary:
        work = Path(temporary)
        runtime = work / "runtime"
        runtime.mkdir(mode=0o700)
        project = work / ("nested-" + "x" * 65) / "app"
        project.mkdir(parents=True)
        relative_project = str(project.relative_to(work))
        (project / "assets").mkdir()
        manifest = json.loads((ROOT / "examples/starter/manifest.json").read_text())
        manifest["assets"] = ["assets/marker.txt"]
        (project / "manifest.json").write_text(json.dumps(manifest))
        (project / "assets/marker.txt").write_text("first")
        source = '<script lang="ts">let count = $state(1); function add() { count += 1; }</script>\n<Column id="content"><Text id="count">{count}</Text><Button id="add" onclick={add}>Add</Button></Column>'
        (project / "app.studio").write_text(source)
        env = os.environ.copy()
        for key in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "SWAYSOCK"):
            env.pop(key, None)
        env.update(XDG_RUNTIME_DIR=str(runtime), WLR_BACKENDS="headless", WLR_RENDERER="pixman", WLR_LIBINPUT_NO_DEVICES="1", LIBGL_ALWAYS_SOFTWARE="1", GALLIUM_DRIVER="llvmpipe")
        compositor_log = (work / "sway.log").open("w")
        app_log = (work / "studio.log").open("w")
        designer_log = (work / "designer-agent.log").open("w")
        compositor = subprocess.Popen(["dbus-run-session", "--", "sway", "-c", str(ROOT / "tests/platform/sway-headless.conf")], env=env, stdout=compositor_log, stderr=subprocess.STDOUT, start_new_session=True)
        app = None
        agent = None
        try:
            wayland = wait_for(lambda: next((p for p in runtime.glob("wayland-*") if p.is_socket()), None), "Wayland socket")
            ipc = wait_for(lambda: next(runtime.glob("sway-ipc.*.sock"), None), "compositor IPC")
            env["WAYLAND_DISPLAY"] = wayland.name
            agent = subprocess.Popen(
                [designer, "agent-server", "--project-root", str(project)],
                cwd=work,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=designer_log,
                text=True,
                bufsize=1,
            )
            initialized = rpc_call(
                agent,
                1,
                "initialize",
                {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {"name": "native-hot-reload-test", "version": "1"},
                },
            )
            assert initialized["serverInfo"]["name"] == "studio-designer"
            agent.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
            agent.stdin.flush()
            app = subprocess.Popen([studio, "dev", relative_project], cwd=work, env=env, stdout=app_log, stderr=subprocess.STDOUT, start_new_session=True)

            def log():
                if app.poll() is not None:
                    raise AssertionError(f"studio exited {app.returncode}:\n{(work / 'studio.log').read_text()}")
                return (work / "studio.log").read_text()

            def window_ids():
                tree = json.loads(subprocess.check_output(["swaymsg", "-s", str(ipc), "-t", "get_tree", "-r"], env=env))
                def collect(node):
                    matches = [node["id"]] if node.get("pid") == app.pid and node.get("type") == "con" else []
                    return matches + [found for child in node.get("nodes", []) + node.get("floating_nodes", []) for found in collect(child)]
                return collect(tree)

            def revision(number):
                wait_for(lambda: f"committed revision {number} (" in log(), f"commit {number}")
                assert window_ids() == original, "reload recreated the native window"

            wait_for(lambda: "committed revision 1 (" in log(), "initial IR mount")
            original = wait_for(window_ids, "native window")
            assert len(original) == 1
            agent_write(agent, 2, "app.studio", source.replace("{count}</Text>", "{count} edited</Text>"))
            revision(2)
            agent_write(agent, 4, "app.studio", '<script lang="ts">let = ;</script>')
            wait_for(lambda: "development compilation failed" in log(), "invalid edit rejection")
            assert "committed revision 3 (" not in log()
            assert window_ids() == original
            agent_write(agent, 6, "app.studio", source)
            revision(3)
            agent_write(agent, 8, "assets/marker.txt", "second")
            revision(4)
            (project / "assembly").mkdir()
            shutil.copy(ROOT / "examples/starter/asconfig.json", project / "asconfig.json")
            entry = (ROOT / "examples/starter/assembly/index.ts").read_text()
            (project / "assembly/index.ts").write_text(entry)
            (project / "app.studio").unlink()
            revision(5)
            agent_write(agent, 10, "assembly/index.ts", entry.replace("count * 125", "count * 250"))
            revision(6)
            preview = tool_call(agent, 12, "preview.status", {})
            assert not preview["isError"], preview
            assert preview["structuredContent"]["running"] is True, preview
            assert preview["structuredContent"]["appliedRevision"] == 6, preview
            restarted = subprocess.run([studio, "restart-session", relative_project], cwd=work, env=env, text=True, capture_output=True, timeout=30)
            assert restarted.returncode == 0, restarted.stderr
            revision(7)
            assert not list((project / "build").glob("*.studio")), "development created a package"
            subprocess.check_output(["swaymsg", "-s", str(ipc), f"[con_id={original[0]}] kill"], env=env)
            assert app.wait(timeout=10) == 0, "closing the native window did not end the dev session cleanly"
            print("Designer MCP/JSON-RPC source edits; native IR edits, rejection/recovery, asset refresh, Wasm edits and restart: ok")
            print("seven committed revisions; one CLI process and native window; zero .studio archives")
        except Exception:
            print((work / "studio.log").read_text())
            print((work / "sway.log").read_text()[-6000:])
            raise
        finally:
            if agent is not None and agent.poll() is None:
                try:
                    agent.stdin.close()
                    agent.wait(timeout=5)
                except (BrokenPipeError, subprocess.TimeoutExpired):
                    agent.terminate()
                    try:
                        agent.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        agent.kill()
                        agent.wait()
            for process in (app, compositor):
                if process is not None and process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
            compositor_log.close()
            app_log.close()
            designer_log.close()


if __name__ == "__main__":
    main()
