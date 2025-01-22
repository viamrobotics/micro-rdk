import asyncio
import time
import copy

from viam.robot.client import DialOptions
from viam.app.viam_client import ViamClient

async def connect(robot_address: str, api_key: str, api_key_id: str) -> ViamClient:
    dial_options = DialOptions.with_api_key(api_key_id=api_key_id,api_key=api_key)
    return await ViamClient.create_from_dial_options(dial_options)

async def main():

    robot_address = os.environ["ESP32_CANARY_ROBOT"]
    api_key = os.environ["ESP32_CANARY_API_KEY"]
    api_key_id = os.environ["ESP32_CANARY_API_KEY_ID"]
    # TODO add to CI
    part_id = os.environ["ESP32_CANARY_ROBOT_PART_ID"]
    # TODO add to CI
    tag_name = os.environ["TAG_NAME"]

    print(f"connecting to robot at {robot_address} ...")

    start = None
    for i in range(5):
        try:
            viam_client = await connect(robot_address, api_key, api_key_id)
            break
        except Exception as e:
            if i == 4:
                raise e
            print(e)
        time.sleep(0.5)

    cloud = viam_client.app_client

    robot_part = await cloud.get_robot_part(robot_part_id=part_id)

    # edit robot part config
    service_updated = False
    updated_config = copy.copy(robot_part.robot_config)
    for service in updated_config["services"]:
        if service["model"] == "ota_service":
            service["attributes"]["version"] = tag_name
            service_updated = True
            print(f"updating OtaServiceConfig to `{service}`")
            break

    if not service_updated:
        viam_client.close()
        sys.exit("failed to update service config")

    await cloud.update_robot_part(
        robot_part_id=robot_part.id,
        name=robot_part.name,
        robot_config=updated_config
    )

    # retrieve new config to verify
    robot_part = await cloud.get_robot_part(robot_part_id=part_id)
    for service in robot_part.robot_config["services"]:
        if service["model"] == "ota_service":
            print(f"OtaServiceConfig after updating: `{service}`")

    viam_client.close()

if __name__ == '__main__':
    asyncio.run(main())
