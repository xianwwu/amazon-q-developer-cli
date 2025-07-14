import os
import shlex
from pathlib import Path

from terminal_bench.agents.installed_agents.abstract_installed_agent import (
    AbstractInstalledAgent,
)
from terminal_bench.terminal.models import TerminalCommand


class AmazonQCLIAgent(AbstractInstalledAgent):

    @staticmethod
    def name() -> str:
        return "Amazon Q CLI"

    def __init__(self, model_name: str | None = None, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._model_name = model_name
        self._start_url = 'https://amzn.awsapps.com/start'
        self.region = 'us-east-1'

    @property
    def _env(self) -> dict[str, str]:
        # SIGv4 = 1 for AWS credentials
        env = {"AMAZON_Q_SIGV4":1}
        return env

    @property
    def _install_agent_script_path(self) -> os.PathLike:
        return Path(__file__).parent / "setup_amazon_q.sh"

    def _run_agent_commands(self, task_description: str) -> list[TerminalCommand]:
        escaped_description = shlex.quote(task_description)
        
        return [
        # q chat with 30 min max timeout and also we wait on input. Using qchat cuz sigv4. 
        # non-interactive for now --> check if needed or not
            TerminalCommand(
                command=f"cargo run --bin chat_cli -- chat --no-interactive --trust-all-tools {escaped_description}",
                max_timeout_sec=1200, 
                block=True,
            )
        ]