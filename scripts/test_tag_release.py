"""Exercise the optional .task with fake Git/GitHub tools; never tag or push."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parent.parent
DOTASK = shutil.which("dotask")
COMMIT = "a" * 40

# Fake commands keep publication impossible even when exercising --push. They
# model command exit boundaries; GitHub's real jq expression is also documented
# in the task so the matching-commit and completed-success requirements are clear.
DRIVER = r'''
import json, os, sys
from pathlib import Path
tool, *args = sys.argv[1:]
with open(os.environ["TASK_TEST_LOG"], "a") as output:
    output.write(json.dumps([tool, *args]) + "\n")
case = os.environ.get("TASK_TEST_CASE", "")
if args == ["--version"]:
    print(tool + " version 1.0.0")
elif tool == "gh":
    assert args[:2] == ["run", "list"], args
    assert args[args.index("--workflow") + 1] == "release.yml"
    assert args[args.index("--branch") + 1] == "main"
    assert args[args.index("--event") + 1] == "push"
    query = args[args.index("--jq") + 1]
    assert '.headSha == "' + "a" * 40 + '"' in query, query
    assert '.status == "completed"' in query, query
    assert '.conclusion == "success"' in query, query
    sys.exit(1 if case == "ci-failure" else 0)
elif args[0] == "diff":
    sys.exit(1 if (case == "dirty" or case == "staged" and "--cached" in args) else 0)
elif args[0] == "log":
    if "--format=format:%H" in args:
        print("a" * 40, end="")
    else:
        print(args[2].replace("--format=", "").replace("%H", "a" * 40).replace("%n", "\n"))
elif args[0] == "fetch":
    assert args == ["fetch", "--no-tags", "origin", "refs/heads/main:refs/remotes/origin/main"]
elif args[0] == "merge-base":
    checks = sum(json.loads(line)[:2] == ["git", "merge-base"]
                 for line in Path(os.environ["TASK_TEST_LOG"]).read_text().splitlines())
    sys.exit(1 if case == "mismatch-" + str(checks) else 0)
elif args[0] == "tag":
    sys.exit(1 if case == "existing-tag" else 0)
elif args[0] == "push":
    sys.exit(1 if case == "push-failure" else 0)
else:
    raise AssertionError(args)
'''


@unittest.skipUnless(DOTASK, "Optional maintainer task checks need installed dotask")
class TagReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="doinstall-task-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / ".tasks").mkdir()
        (self.root / "scripts").mkdir()
        shutil.copy2(ROOT / "scripts/release.py", self.root / "scripts")
        (self.root / "Cargo.toml").write_text('[package]\nversion = "0.1.0"\n')
        tools = self.root / "tools"
        tools.mkdir()
        driver = tools / "driver.py"
        driver.write_text(DRIVER)
        for tool in ("git", "gh"):
            if os.name == "nt":
                (tools / (tool + ".cmd")).write_text(f'@"{sys.executable}" -B "{driver}" {tool} %*\n')
            else:
                script = tools / tool
                script.write_text(f'#!{sys.executable}\n' + DRIVER.replace(
                    'tool, *args = sys.argv[1:]', f'tool, args = "{tool}", sys.argv[1:]'))
                script.chmod(0o755)
        # Bind the two external commands explicitly: Windows otherwise prefers
        # an existing git.exe over a PATH-local git.cmd. The workflow itself is
        # unchanged, and no test can accidentally reach real tag/push commands.
        task = (ROOT / ".tasks/tag-release.task").read_text()
        for tool in ("git", "gh"):
            path = tools / (tool + (".cmd" if os.name == "nt" else ""))
            task = task.replace("executable: " + tool, "executable: '" + path.as_posix() + "'")
            task = task.replace("value: " + tool + "}", "value: python}")
        (self.root / ".tasks/tag-release.task").write_text(task)
        self.log = self.root / "commands.jsonl"
        self.env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                        TASK_TEST_LOG=str(self.log))

    def invoke(self, *arguments, case="", success=True):
        self.env["TASK_TEST_CASE"] = case
        result = subprocess.run([DOTASK, "tag-release", *arguments], cwd=self.root,
                                env=self.env, capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_preview_never_tags_or_pushes_even_with_push_option(self):
        commands = self.invoke("--create=false", "--push")
        self.assertTrue(any(command[:3] == ["gh", "run", "list"] for command in commands))
        self.assertFalse(any(command[:2] in (["git", "tag"], ["git", "push"]) for command in commands))

    def test_default_creates_annotated_tag_at_verified_commit_without_push(self):
        commands = self.invoke()
        self.assertEqual(commands[-1], ["git", "tag", "-a", "v0.1.0", COMMIT, "-m", "doinstall v0.1.0"])
        self.assertFalse(any(command[:2] == ["git", "push"] for command in commands))

    def test_push_sends_only_new_release_tag(self):
        commands = self.invoke("--push")
        self.assertEqual(commands[-2][:4], ["git", "tag", "-a", "v0.1.0"])
        self.assertEqual(commands[-1], ["git", "push", "origin", "refs/tags/v0.1.0"])

    def test_failed_guards_stop_before_tagging(self):
        for case in ("dirty", "staged", "ci-failure", *("mismatch-" + str(n) for n in range(1, 7))):
            with self.subTest(case=case):
                self.log.write_text("")
                commands = self.invoke("--push", case=case, success=False)
                self.assertFalse(any(command[:2] in (["git", "tag"], ["git", "push"]) for command in commands))

    def test_invalid_cargo_version_stops_before_tagging(self):
        (self.root / "Cargo.toml").write_text('[package]\nversion = "0.1.0-preview"\n')
        commands = self.invoke("--push", success=False)
        self.assertFalse(any(command[:2] in (["git", "tag"], ["git", "push"]) for command in commands))

    def test_existing_tag_refuses_push(self):
        commands = self.invoke("--push", case="existing-tag", success=False)
        self.assertEqual(commands[-1][:2], ["git", "tag"])

    def test_push_failure_does_not_delete_or_retarget_tag(self):
        commands = self.invoke("--push", case="push-failure", success=False)
        self.assertEqual(commands[-1], ["git", "push", "origin", "refs/tags/v0.1.0"])
        self.assertEqual(sum(command[:2] == ["git", "tag"] for command in commands), 1)


if __name__ == "__main__":
    unittest.main()
