export default {
  "*.{rs,toml}": () => [
    "cargo +nightly fmt --check -- --color always",
    "cargo clippy --locked --color always -- -D warnings",
  ],
  "*.py": ["ruff format --check", "ruff check"],
  "*.{ts,js,tsx,jsx,mjs}": "prettier --check",
  "!(*test*)*": "typos",
};
