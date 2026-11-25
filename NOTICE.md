# NOTICE

## trafficlights_webapp — license and provenance

Repository: <https://github.com/NB-TUDelft/trafficlights_webapp>

Copyright (C) 2025-2026 Zamkorus (Krzysztof Zabłocki), NB-TUDelft, and trafficlights_webapp contributors

This program as a whole is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by the
Free Software Foundation, either version 3 of the License, or (at your option)
any later version (SPDX: `AGPL-3.0-or-later`).

This program is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License along
with this program (see [LICENSE](LICENSE)). If not, see
<https://www.gnu.org/licenses/>.

Corresponding Source for deployed network instances (AGPL Section 13) is
available at <https://github.com/NB-TUDelft/trafficlights_webapp>.

## Contributors

Each contributor holds copyright in their own contributions, licensed as part
of the AGPL-3.0-or-later whole:

  Krzysztof Zabłocki (Zamkorus)                    2025-12-04 to present

## Hardware and protocol compatibility

This application interoperates with the classroom traffic-light micro:bit
hardware and legacy serial wire protocol originally introduced by
[RolfHut/ClassroomIndicatorLights](https://github.com/RolfHut/ClassroomIndicatorLights),
as well as the replacement firmware in `trafficlights_firmware`. No source code
from `RolfHut/ClassroomIndicatorLights` is incorporated in this repository;
both the starting `fastapi_receiver.py` prototype and the Rust/web application
are original works under `AGPL-3.0-or-later`.
