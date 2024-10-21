<div align="center">

# Smart Blinds

Simple auto blinds framework demo

</div>

## Design

The Smart Blinds system is designed to manage and automate the operation of
window blinds based on environmental data collected by sensors. The system
architecture includes multiple user groups, each containing several regions,
with each region capable of controlling multiple windows and sensors. This
modular design allows for scalable and customizable control over various
environments. At the same time, we have a user layer management, users can
schedule the desired brightness at different times through the gantt graph.

![room.png](documents/room.png)

- Group: Represents a collection of users and sensors in a defined area.
- Admin: The user with administrative privileges who can oversee and manage the
  entire group.
- User A and User B: Regular users who control specific regions within the group.
- Sensors(A, B, C): Devices placed in various locations to collect data on light
  and temperature.
- Blinds: Mechanisms controlled by the system to adjust based on sensor data.

The following diagram illustrates the relationships between the different
entities within the system:

```mermaid
erDiagram

groups { int id "PK" string name "UK" }
users { int id "PK" int group_id "FK" string email "UK" string password string role }
regions { int id "PK" int group_id "FK" string name "UK" int light float temperature }
settings { int id "PK" int user_id "FK" int light float temperature datetime start datetime end int interval }
windows { int id "PK" int region_id "FK" string name "UK" float state }
sensors { int id "PK" int region_id "FK" string name "UK" }
sensor_data { int id "PK" int sensor_id "FK" int light float temperature datetime time }
users_regions_link { int id "PK" int user_id "FK" int region_id "FK" }
regions_settings_link { int id "PK" int region_id "FK" int setting_id "FK" }

groups ||--|{ users : "group_id"
groups ||--|{ regions : "group_id"
users ||--|{ settings : "user_id"
regions ||--o{ windows: "region_id"
regions ||--|{ sensors : "region_id"
users ||--|{ users_regions_link : "user_id"
regions ||--|{ users_regions_link : "region_id"
sensors ||--o{ sensor_data : "sensor_id"
regions ||--|{ regions_settings_link : "region_id"
settings ||--|{ regions_settings_link : "setting_id"
```

## Assembly

Follow these steps to assemble the sensor and blinds correctly.

### Materials

- 2x Arduino * (nano)
- 28BYJ-48 stepper motor
- Light dependent resistor(LDR) + Resistor for LDR
- NTC resistor + Resistor for NTC

### Procedural

1. Assemble the Stepper Motor:

   - Attach the 28BYJ-48 stepper motor to the Arduino Nano using appropriate
     driver circuitry.

   - Ensure the motor is firmly mounted to control the blinds mechanism.

   - Assemble conductive devices to connect blinds or curtains.

```mermaid
---
config:
  layout: elk
  look: handDrawn
---
flowchart LR
    subgraph VCC5["+5 V USB"]
        VCC5_1(("+5 V"))
    end
    subgraph GND["Ground"]
        GND_1(("GND"))
    end
    subgraph MCU["ESP32"]
        direction TB
            POW("ss")
            GPIO25("GPIO25")
            GPIO26("GPIO26")
            GPIO32("GPIO32")
            GPIO33("GPIO33")
            MCU_GND(("GND"))
    end
    subgraph USBTTL["HW-597  (CH340)"]
        direction TB
            CH_IN_5V["+5 V IN"]
            CH_OUT_5V("+5 V OUT")
            CH_GND("GND")
    end
    subgraph DRIVER["ULN2003 + 28BYJ-48"]
        direction TB
            IN1("IN1")
            IN2("IN2")
            IN3("IN3")
            IN4("IN4")
            A("PIN A")
            B("PIN B")
            C("PIN C")
            D("PIN D")
            ULN_VCC("+5 V")
            ULN_GND("GND")
    end
    GPIO25 -- IN1 --> IN1
    GPIO26 -- IN2 --> IN2
    GPIO32 -- IN3 --> IN3
    GPIO33 -- IN4 --> IN4
    IN1 -- |OUT1| --> A
    IN2 -- |OUT2| --> B
    IN3 -- |OUT3| --> C
    IN4 -- |OUT4| --> D
    VCC5_1 --- CH_IN_5V & POW
    GND_1 --- MCU_GND & CH_GND & ULN_GND
    CH_OUT_5V --- ULN_VCC
```

> [!NOTE]  
> If using ready-made sensor provider can skip the `step 2`

2. Assemble the Sensor:

   - Connect the LDR and its corresponding resistor to an analog pin on the
   Arduino Nano.

   - Connect the NTC resistor and its corresponding resistor to another analog
   pin on the Arduino Nano.

   - Ensure all connections are secure and insulated.

   ![sensor.png](documents/sensor.png)


## Development

**Option 1: One click start**

For develop frontend or embedded side, could try one click start:

```bash
python bootstrap.py
```

**Option 2: Manual start**

To start the server, you will need `Rust` installed on your machine. Then, run
the following command:

```bash
cargo run --package lumisync-server --bin server
```

To work on the web application, you should have `Node.js(LTS version)` and `npm`
installed. Then, run the following command:

```bash
npm install
npm run web
```

**Database setup**

1. Create the database (require url format, for example `sqlite:debug.db`) and
   add migration script.

   ```bash
   sqlx database create --database-url <url>
   sqlx migrate add <name>
   ```

2. Check the new migration script under `migrations/<timestamp>_<name>.sql`. Add
   custom database schema changes to this file.

   ```sql
   -- Insert sample data into 'groups'
   INSERT INTO groups (name) VALUES ('sample');

   -- Insert sample data into 'users'
   -- Password: test
   INSERT INTO users (group_id, email, password, role) VALUES (1, 'test@test.com', '$argon2id$v=19$m=19456,t=2,p=1$zk5JmuovvG7B6vyGGmLxDQ$qoqCpKkqrgoVjeTGa5ewrqFpuPUisTCDnEiPz6Dh/oc', 'admin');

   -- Insert sample data into 'regions'
   INSERT INTO regions (group_id, name, light, temperature) VALUES (1, 'Living Room', 100, 22.5);

   -- Insert sample data into 'settings'
   INSERT INTO settings (user_id, light, temperature, start, end, interval) VALUES (1, 100, 22.5, DATETIME('now'), DATETIME('now', '+03:30'), 0);

   -- Insert sample data into 'windows'
   INSERT INTO windows (region_id, name, state) VALUES (1, 'Living Room Right Window', 0);

   -- Insert sample data into 'sensors'
   INSERT INTO sensors (region_id, name) VALUES (1, 'SENSOR-MOCK');

   -- Insert sample data into 'users_regions_link'
   INSERT INTO users_regions_link (user_id, region_id) VALUES (1, 1);

   -- Insert sample data into 'regions_settings_link'
   INSERT INTO regions_settings_link (region_id, setting_id) VALUES (1, 1);
   ```

3. Adjust config file for `configs/development.toml` with database url.

   ```toml
   [database]
   migration_path = "migrations"
   clean_start = true # or false - it dependenies on your migration
   url = "<url>"
   ```

**Connect service provider**

1. For connecting to sensor service provider, gateway related setting need be
   adjusted:

   ```toml
   [gateway]
   host = "...amazonaws.com"
   port = 8883

   [gateway.topic]
   prefix_type = "json"
   prefix_mode = "pr"
   prefix_country = "fi"

   [gateway.auth]
   cert_path = "configs/cloud.pem" # Replace with your certification
   key_path = "configs/cloud.key" # Replace with your private key
   ```

2. Modified the group name by new migration or edit the database directly:

   ```sql
   UPDATE groups SET name = 'MY_CUSTOMER_ID' WHERE id = 1;
   ```
