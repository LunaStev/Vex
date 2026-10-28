# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0
# RUNTIME_IMAGE comes only from the immutable platform table.
ARG RUNTIME_IMAGE=debian@sha256:e3b4cb1b0056896c4b43e17472b906b10007ab94bc533d37a47e9a5795eebbc0
FROM ${RUNTIME_IMAGE}
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    ca-certificates curl git python3 python3-venv ninja-build build-essential pkg-config gh procps \
    && rm -rf /var/lib/apt/lists/*
COPY requirements-build.txt /tmp/requirements-build.txt
RUN python3 -m venv /opt/vex-meson \
    && /opt/vex-meson/bin/pip install --no-cache-dir -r /tmp/requirements-build.txt \
    && ln -s /opt/vex-meson/bin/meson /usr/local/bin/meson
