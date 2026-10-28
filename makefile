# tags the version in Cargo.toml and pushes it, which starts release.yml
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

release:
	@test "$$(git branch --show-current)" = main || (echo "not on main" && exit 1)
	@test -z "$$(git status --porcelain)" || (echo "working tree not clean" && exit 1)
	@git fetch -q origin main
	@test "$$(git rev-parse HEAD)" = "$$(git rev-parse origin/main)" || (echo "main is not in sync with origin, run git pull" && exit 1)
	@grep -A1 '^name = "ld2410s"' Cargo.lock | grep -q 'version = "$(VERSION)"' || (echo "Cargo.lock is not at $(VERSION), run cargo build" && exit 1)
	git tag -a v$(VERSION) -m v$(VERSION)
	git push origin v$(VERSION)
