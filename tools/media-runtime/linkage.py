"""Conservative direct native-runtime dependency allowlist."""
def system_dependencies(system, dependencies):
    if system == "Darwin":
        return all(d.startswith(("/usr/lib/", "/System/Library/")) for d in dependencies)
    if system == "Linux":
        allowed = {"libc.so.6", "libm.so.6", "libpthread.so.0", "libdl.so.2",
                   "librt.so.1", "libgcc_s.so.1", "ld-linux-x86-64.so.2",
                   "ld-linux-aarch64.so.1", "linux-vdso.so.1"}
        return all(d in allowed for d in dependencies)
    if system == "Windows":
        allowed = {"kernel32.dll", "user32.dll", "advapi32.dll", "bcrypt.dll",
                   "secur32.dll", "ws2_32.dll", "ole32.dll", "shell32.dll",
                   "msvcrt.dll", "ucrtbase.dll", "ntdll.dll"}
        return all(d.lower() in allowed or d.lower().startswith("api-ms-win-")
                   for d in dependencies)
    return False
