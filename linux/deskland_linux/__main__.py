from .gpu import configure_single_gpu_env

configure_single_gpu_env()

from .app import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main())
