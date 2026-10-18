import unittest
from linkage import system_dependencies


class LinkageTests(unittest.TestCase):
    def test_only_system_dependencies(self):
        self.assertTrue(system_dependencies("Darwin", ["/usr/lib/libSystem.B.dylib"]))
        self.assertFalse(system_dependencies("Darwin", ["/opt/homebrew/lib/libavcodec.dylib"]))
        self.assertTrue(system_dependencies("Windows", ["KERNEL32.dll", "ucrtbase.dll"]))
        self.assertFalse(system_dependencies("Windows", ["libwinpthread-1.dll"]))
        self.assertTrue(system_dependencies("Linux", ["libc.so.6", "libm.so.6"]))
        self.assertFalse(system_dependencies("Linux", ["libavcodec.so.62"]))


if __name__ == "__main__":
    unittest.main()
