#!/usr/bin/env python
# -*- coding: utf-8 -*-
# try to use system python to start tests to avoid any installation workarounds. You can use another version via docker, etc,...

try:
    from pydocker import DockerFile  # pip install -U pydocker
except ImportError:
    try:
        from urllib.request import urlopen         # python-3
    except ImportError:
        from urllib import urlopen  # python-2
    #
    exec(urlopen('https://raw.githubusercontent.com/jen-soft/pydocker/master/pydocker.py').read())
#
import sys
import logging

import subprocess

logging.getLogger('').setLevel(logging.INFO)
logging.root.addHandler(logging.StreamHandler(sys.stdout))


class MyDockerFile(DockerFile):
    """   add here your custom features   """
    # FROM = "centos:centos7"
    RUN = "mkdir -p /tmp/myuser"
    WORKDIR = "/tmp/myuser"
    CMD = ["/bin/bash", "echo", "Hello:)))"]

    def run(self):
        p_args = ["docker", "run", "--name", "TEST_TMP", "--rm", self.get_img_name()]
        print("Command: >", " ".join(p_args))
        p = subprocess.Popen(
            p_args, 
            # shell=True,
            stdout=subprocess.PIPE, 
            stderr=subprocess.PIPE,
            # stdout=sys.stdout, 
            # stderr=sys.stderr,
            # stdin=subprocess.PIPE,
        )
        res = p.communicate()
        print("*", res)
        return p.returncode

#


d = MyDockerFile(base_img="centos:centos7", name="local/test:0.0.1")
# ...
d.build_img()
d.run()
