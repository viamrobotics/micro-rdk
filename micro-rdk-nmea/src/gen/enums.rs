// AUTO-GENERATED CODE; DO NOT DELETE OR EDIT
use crate::define_nmea_enum;
use crate::parse_helpers::enums::NmeaEnumeratedField;

define_nmea_enum!(
    AcceptabilityLookup,
    (1, BadFrequency, "Bad frequency"),
    (0, BadLevel, "Bad level"),
    (2, BeingQualified, "Being qualified"),
    (3, Good, "Good"),
    UnknownLookupField
);
define_nmea_enum!(
    AccessLevelLookup,
    (0, Locked, "Locked"),
    (1, UnlockedLevel1, "unlocked level 1"),
    (2, UnlockedLevel2, "unlocked level 2"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarCalibrateFunctionLookup,
    (1, EnterCalibrationMode, "Enter calibration mode"),
    (0, NormalcancelCalibration, "Normal/cancel calibration"),
    (2, ResetCalibrationTo0, "Reset calibration to 0"),
    (4, ResetCompassToDefaults, "Reset compass to defaults"),
    (5, ResetDampingToDefaults, "Reset damping to defaults"),
    (3, Verify, "Verify"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarCalibrateStatusLookup,
    (4, FailedOther, "Failed - other"),
    (3, FailedTiltError, "Failed - tilt error"),
    (2, FailedTimeout, "Failed - timeout"),
    (5, InProgress, "In progress"),
    (1, Passed, "Passed"),
    (0, Queried, "Queried"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarCommandLookup,
    (32, AttitudeOffsets, "Attitude Offsets"),
    (33, CalibrateCompass, "Calibrate Compass"),
    (40, CalibrateDepth, "Calibrate Depth"),
    (41, CalibrateSpeed, "Calibrate Speed"),
    (42, CalibrateTemperature, "Calibrate Temperature"),
    (46, Nmea2000Options, "NMEA 2000 options"),
    (35, SimulateMode, "Simulate Mode"),
    (43, SpeedFilter, "Speed Filter"),
    (44, TemperatureFilter, "Temperature Filter"),
    (34, TrueWindOptions, "True Wind Options"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarDepthQualityFactorLookup,
    (0, DepthUnlocked, "Depth unlocked"),
    (1, Quality10, "Quality 10%"),
    (10, Quality100, "Quality 100%"),
    (2, Quality20, "Quality 20%"),
    (3, Quality30, "Quality 30%"),
    (4, Quality40, "Quality 40%"),
    (5, Quality50, "Quality 50%"),
    (6, Quality60, "Quality 60%"),
    (7, Quality70, "Quality 70%"),
    (8, Quality80, "Quality 80%"),
    (9, Quality90, "Quality 90%"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarFilterLookup,
    (1, BasicIirFilter, "Basic IIR filter"),
    (0, NoFilter, "No filter"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarPostControlLookup,
    (1, GenerateNewValues, "Generate new values"),
    (0, ReportPreviousValues, "Report previous values"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarPostIdLookup,
    (8, BatteryVoltageSensor, "Battery voltage sensor"),
    (2, FactoryEeprom, "Factory EEPROM"),
    (1, FormatCode, "Format Code"),
    (7, InternalTemperatureSensor, "Internal temperature sensor"),
    (5, SonarTransceiver, "Sonar Transceiver"),
    (6, SpeedSensor, "Speed sensor"),
    (3, UserEeprom, "User EEPROM"),
    (4, WaterTemperatureSensor, "Water Temperature Sensor"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarTemperatureInstanceLookup,
    (0, DeviceSensor, "Device Sensor"),
    (1, OnboardWaterSensor, "Onboard Water Sensor"),
    (2, OptionalWaterSensor, "Optional Water Sensor"),
    UnknownLookupField
);
define_nmea_enum!(
    AirmarTransmissionIntervalLookup,
    (0, MeasureInterval, "Measure interval"),
    (1, RequestedByUser, "Requested by user"),
    UnknownLookupField
);
define_nmea_enum!(
    AisAssignedModeLookup,
    (1, AssignedMode, "Assigned mode"),
    (0, AutonomousAndContinuous, "Autonomous and continuous"),
    UnknownLookupField
);
define_nmea_enum!(
    AisBandLookup,
    (1, EntireMarineBand, "Entire marine band"),
    (0, Top525KHzOfMarineBand, "Top 525 kHz of marine band"),
    UnknownLookupField
);
define_nmea_enum!(
    AisCommunicationStateLookup,
    (1, Itdma, "ITDMA"),
    (0, Sotdma, "SOTDMA"),
    UnknownLookupField
);
define_nmea_enum!(
    AisMessageIdLookup,
    (21, AtonReport, "ATON report"),
    (
        2,
        AssignedScheduledClassAPositionReport,
        "Assigned scheduled Class A position report"
    ),
    (16, AssignmentModeCommand, "Assignment mode command"),
    (4, BaseStationReport, "Base station report"),
    (7, BinaryAcknowledgement, "Binary acknowledgement"),
    (6, BinaryAddressedMessage, "Binary addressed message"),
    (8, BinaryBroadcastMessage, "Binary broadcast message"),
    (22, ChannelManagement, "Channel management"),
    (
        17,
        DgnssBroadcastBinaryMessage,
        "DGNSS broadcast binary message"
    ),
    (
        20,
        DataLinkManagementMessage,
        "Data link management message"
    ),
    (
        19,
        ExtendedClassBPositionReport,
        "Extended Class B position report"
    ),
    (23, GroupAssignmentCommand, "Group assignment command"),
    (
        3,
        InterrogatedClassAPositionReport,
        "Interrogated Class A position report"
    ),
    (15, Interrogation, "Interrogation"),
    (
        26,
        MultipleSlotBinaryMessage,
        "Multiple slot binary message"
    ),
    (
        27,
        PositionReportForLongRangeApplications,
        "Position report for long range applications"
    ),
    (
        13,
        SafetyRelatedAcknowledgement,
        "Safety related acknowledgement"
    ),
    (
        12,
        SafetyRelatedAddressedMessage,
        "Safety related addressed message"
    ),
    (
        14,
        SatetyRelatedBroadcastMessage,
        "Satety related broadcast message"
    ),
    (
        1,
        ScheduledClassAPositionReport,
        "Scheduled Class A position report"
    ),
    (25, SingleSlotBinaryMessage, "Single slot binary message"),
    (
        18,
        StandardClassBPositionReport,
        "Standard Class B position report"
    ),
    (
        9,
        StandardSarAircraftPositionReport,
        "Standard SAR aircraft position report"
    ),
    (
        5,
        StaticAndVoyageRelatedData,
        "Static and voyage related data"
    ),
    (24, StaticDataReport, "Static data report"),
    (10, UtCdateInquiry, "UTC/date inquiry"),
    (11, UtCdateResponse, "UTC/date response"),
    UnknownLookupField
);
define_nmea_enum!(
    AisModeLookup,
    (1, Assigned, "Assigned"),
    (0, Autonomous, "Autonomous"),
    UnknownLookupField
);
define_nmea_enum!(
    AisSpecialManeuverLookup,
    (2, EngagedInSpecialManeuver, "Engaged in special maneuver"),
    (0, NotAvailable, "Not available"),
    (
        1,
        NotEngagedInSpecialManeuver,
        "Not engaged in special maneuver"
    ),
    (3, Reserved, "Reserved"),
    UnknownLookupField
);
define_nmea_enum!(
    AisTransceiverLookup,
    (0, ChannelAVdlReception, "Channel A VDL reception"),
    (2, ChannelAVdlTransmission, "Channel A VDL transmission"),
    (1, ChannelBVdlReception, "Channel B VDL reception"),
    (3, ChannelBVdlTransmission, "Channel B VDL transmission"),
    (
        4,
        OwnInformationNotBroadcast,
        "Own information not broadcast"
    ),
    (5, Reserved, "Reserved"),
    UnknownLookupField
);
define_nmea_enum!(
    AisTypeLookup,
    (1, Cs, "CS"),
    (0, Sotdma, "SOTDMA"),
    UnknownLookupField
);
define_nmea_enum!(
    AisVersionLookup,
    (3, IturM1371FutureEdition, "ITU-R M.1371 future edition"),
    (0, IturM13711, "ITU-R M.1371-1"),
    (1, IturM13713, "ITU-R M.1371-3"),
    (2, IturM13715, "ITU-R M.1371-5"),
    UnknownLookupField
);
define_nmea_enum!(
    AlertCategoryLookup,
    (0, Navigational, "Navigational"),
    (1, Technical, "Technical"),
    UnknownLookupField
);
define_nmea_enum!(
    AlertLanguageIdLookup,
    (2, Arabic, "Arabic"),
    (3, ChineseSimplified, "Chinese (simplified)"),
    (4, Croatian, "Croatian"),
    (5, Danish, "Danish"),
    (6, Dutch, "Dutch"),
    (1, EnglishUk, "English (UK)"),
    (0, EnglishUs, "English (US)"),
    (7, Finnish, "Finnish"),
    (8, French, "French"),
    (9, German, "German"),
    (10, Greek, "Greek"),
    (11, Italian, "Italian"),
    (12, Japanese, "Japanese"),
    (13, Korean, "Korean"),
    (14, Norwegian, "Norwegian"),
    (15, Polish, "Polish"),
    (16, Portuguese, "Portuguese"),
    (17, Russian, "Russian"),
    (18, Spanish, "Spanish"),
    (19, Swedish, "Swedish"),
    UnknownLookupField
);
define_nmea_enum!(
    AlertResponseCommandLookup,
    (0, Acknowledge, "Acknowledge"),
    (1, TemporarySilence, "Temporary Silence"),
    (2, TestCommandOff, "Test Command off"),
    (3, TestCommandOn, "Test Command on"),
    UnknownLookupField
);
define_nmea_enum!(
    AlertStateLookup,
    (4, Acknowledged, "Acknowledged"),
    (2, Active, "Active"),
    (5, AwaitingAcknowledge, "Awaiting Acknowledge"),
    (0, Disabled, "Disabled"),
    (1, Normal, "Normal"),
    (3, Silenced, "Silenced"),
    UnknownLookupField
);
define_nmea_enum!(
    AlertThresholdStatusLookup,
    (4, Acknowledged, "Acknowledged"),
    (5, AwaitingAcknowledge, "Awaiting Acknowledge"),
    (2, ExtremeThresholdExceeded, "Extreme Threshold Exceeded"),
    (3, LowThresholdExceeded, "Low Threshold Exceeded"),
    (0, Normal, "Normal"),
    (1, ThresholdExceeded, "Threshold Exceeded"),
    UnknownLookupField
);
define_nmea_enum!(
    AlertTriggerConditionLookup,
    (1, Auto, "Auto"),
    (3, Disabled, "Disabled"),
    (0, Manual, "Manual"),
    (2, Test, "Test"),
    UnknownLookupField
);
define_nmea_enum!(
    AlertTypeLookup,
    (2, Alarm, "Alarm"),
    (8, Caution, "Caution"),
    (1, EmergencyAlarm, "Emergency Alarm"),
    (5, Warning, "Warning"),
    UnknownLookupField
);
define_nmea_enum!(
    AtonTypeLookup,
    (
        0,
        DefaultTypeOfAtoNNotSpecified,
        "Default: Type of AtoN not specified"
    ),
    (10, FixedBeaconCardinalE, "Fixed beacon: cardinal E"),
    (9, FixedBeaconCardinalN, "Fixed beacon: cardinal N"),
    (11, FixedBeaconCardinalS, "Fixed beacon: cardinal S"),
    (12, FixedBeaconCardinalW, "Fixed beacon: cardinal W"),
    (
        17,
        FixedBeaconIsolatedDanger,
        "Fixed beacon: isolated danger"
    ),
    (13, FixedBeaconPortHand, "Fixed beacon: port hand"),
    (
        15,
        FixedBeaconPreferredChannelPortHand,
        "Fixed beacon: preferred channel port hand"
    ),
    (
        16,
        FixedBeaconPreferredChannelStarboardHand,
        "Fixed beacon: preferred channel starboard hand"
    ),
    (18, FixedBeaconSafeWater, "Fixed beacon: safe water"),
    (19, FixedBeaconSpecialMark, "Fixed beacon: special mark"),
    (14, FixedBeaconStarboardHand, "Fixed beacon: starboard hand"),
    (7, FixedLeadingLightFront, "Fixed leading light front"),
    (8, FixedLeadingLightRear, "Fixed leading light rear"),
    (6, FixedLightWithSectors, "Fixed light: with sectors"),
    (5, FixedLightWithoutSectors, "Fixed light: without sectors"),
    (3, FixedStructureOffshore, "Fixed structure off-shore"),
    (21, FloatingAtoNCardinalE, "Floating AtoN: cardinal E"),
    (20, FloatingAtoNCardinalN, "Floating AtoN: cardinal N"),
    (22, FloatingAtoNCardinalS, "Floating AtoN: cardinal S"),
    (23, FloatingAtoNCardinalW, "Floating AtoN: cardinal W"),
    (
        28,
        FloatingAtoNIsolatedDanger,
        "Floating AtoN: isolated danger"
    ),
    (
        31,
        FloatingAtoNLightVesselLanbYrigs,
        "Floating AtoN: light vessel/LANBY/rigs"
    ),
    (
        24,
        FloatingAtoNPortHandMark,
        "Floating AtoN: port hand mark"
    ),
    (
        26,
        FloatingAtoNPreferredChannelPortHand,
        "Floating AtoN: preferred channel port hand"
    ),
    (
        27,
        FloatingAtoNPreferredChannelStarboardHand,
        "Floating AtoN: preferred channel starboard hand"
    ),
    (29, FloatingAtoNSafeWater, "Floating AtoN: safe water"),
    (30, FloatingAtoNSpecialMark, "Floating AtoN: special mark"),
    (
        25,
        FloatingAtoNStarboardHandMark,
        "Floating AtoN: starboard hand mark"
    ),
    (2, Racon, "RACON"),
    (1, ReferencePoint, "Reference point"),
    (4, ReservedForFutureUse, "Reserved for future use"),
    UnknownLookupField
);
define_nmea_enum!(
    AvailableLookup,
    (0, Available, "Available"),
    (1, NotAvailable, "Not available"),
    UnknownLookupField
);
define_nmea_enum!(
    BandgDecimalsLookup,
    (0, UnformattableVariantA, "0"),
    (1, UnformattableVariantB, "1"),
    (2, UnformattableVariantC, "2"),
    (3, UnformattableVariantD, "3"),
    (4, UnformattableVariantE, "4"),
    (254, Auto, "Auto"),
    UnknownLookupField
);
define_nmea_enum!(
    BatteryChemistryLookup,
    (1, Li, "Li"),
    (2, NiCd, "NiCd"),
    (4, NiMh, "NiMH"),
    (0, PbLead, "Pb (Lead)"),
    (3, ZnO, "ZnO"),
    UnknownLookupField
);
define_nmea_enum!(
    BatteryTypeLookup,
    (2, Agm, "AGM"),
    (0, Flooded, "Flooded"),
    (1, Gel, "Gel"),
    UnknownLookupField
);
define_nmea_enum!(
    BatteryVoltageLookup,
    (1, UnformattableVariantA, "12V"),
    (2, UnformattableVariantB, "24V"),
    (3, UnformattableVariantC, "32V"),
    (4, UnformattableVariantD, "36V"),
    (5, UnformattableVariantE, "42V"),
    (6, UnformattableVariantF, "48V"),
    (0, UnformattableVariantG, "6V"),
    UnknownLookupField
);
define_nmea_enum!(
    BearingModeLookup,
    (0, GreatCircle, "Great Circle"),
    (1, Rhumbline, "Rhumbline"),
    UnknownLookupField
);
define_nmea_enum!(
    BluetoothSourceStatusLookup,
    (1, Connected, "Connected"),
    (2, Connecting, "Connecting"),
    (3, NotConnected, "Not connected"),
    (0, Reserved, "Reserved"),
    UnknownLookupField
);
define_nmea_enum!(
    BluetoothStatusLookup,
    (0, Connected, "Connected"),
    (1, NotConnected, "Not connected"),
    (2, NotPaired, "Not paired"),
    UnknownLookupField
);
define_nmea_enum!(
    BootStateLookup,
    (0, InStartupMonitor, "in Startup Monitor"),
    (2, RunningApplication, "running Application"),
    (1, RunningBootloader, "running Bootloader"),
    UnknownLookupField
);
define_nmea_enum!(
    ChargerModeLookup,
    (3, Echo, "Echo"),
    (1, Primary, "Primary"),
    (2, Secondary, "Secondary"),
    (0, Standalone, "Standalone"),
    UnknownLookupField
);
define_nmea_enum!(
    ChargerStateLookup,
    (2, Absorption, "Absorption"),
    (1, Bulk, "Bulk"),
    (7, ConstantVi, "Constant VI"),
    (8, Disabled, "Disabled"),
    (4, Equalise, "Equalise"),
    (9, Fault, "Fault"),
    (5, Float, "Float"),
    (6, NoFloat, "No float"),
    (0, NotCharging, "Not charging"),
    (3, Overcharge, "Overcharge"),
    UnknownLookupField
);
define_nmea_enum!(
    ChargingAlgorithmLookup,
    (2, UnformattableVariantA, "2 stage (no float)"),
    (3, UnformattableVariantB, "3 stage"),
    (
        1,
        ConstantVoltageConstantCurrent,
        "Constant voltage / Constant current"
    ),
    (0, Trickle, "Trickle"),
    UnknownLookupField
);
define_nmea_enum!(
    ControllerStateLookup,
    (2, BusOff, "Bus Off"),
    (0, ErrorActive, "Error Active"),
    (1, ErrorPassive, "Error Passive"),
    UnknownLookupField
);
define_nmea_enum!(
    ConverterStateLookup,
    (4, Absorption, "Absorption"),
    (10, Assisting, "Assisting"),
    (3, Bulk, "Bulk"),
    (7, Equalize, "Equalize"),
    (2, Fault, "Fault"),
    (5, Float, "Float"),
    (9, Inverting, "Inverting"),
    (1, LowPowerMode, "Low Power Mode"),
    (0, Off, "Off"),
    (8, PassThru, "Pass thru"),
    (6, Storage, "Storage"),
    UnknownLookupField
);
define_nmea_enum!(
    DcSourceLookup,
    (1, Alternator, "Alternator"),
    (0, Battery, "Battery"),
    (2, Convertor, "Convertor"),
    (3, SolarCell, "Solar cell"),
    (4, WindGenerator, "Wind generator"),
    UnknownLookupField
);
define_nmea_enum!(
    DeviceClassLookup,
    (70, Communication, "Communication"),
    (
        100,
        DeckCargoFishingEquipmentSystems,
        "Deck + cargo + fishing equipment systems"
    ),
    (120, Display, "Display"),
    (30, ElectricalDistribution, "Electrical Distribution"),
    (35, ElectricalGeneration, "Electrical Generation"),
    (125, Entertainment, "Entertainment"),
    (85, ExternalEnvironment, "External Environment"),
    (110, HumanInterface, "Human Interface"),
    (
        80,
        InstrumentationgeneralSystems,
        "Instrumentation/general systems"
    ),
    (90, InternalEnvironment, "Internal Environment"),
    (25, InternetworkDevice, "Internetwork device"),
    (60, Navigation, "Navigation"),
    (50, Propulsion, "Propulsion"),
    (0, ReservedFor2000Use, "Reserved for 2000 Use"),
    (20, SafetySystems, "Safety systems"),
    (
        75,
        SensorCommunicationInterface,
        "Sensor Communication Interface"
    ),
    (
        40,
        SteeringAndControlSurfaces,
        "Steering and Control surfaces"
    ),
    (10, SystemTools, "System tools"),
    UnknownLookupField
);
define_nmea_enum!(
    DeviceTempStateLookup,
    (0, Cold, "Cold"),
    (2, Hot, "Hot"),
    (1, Warm, "Warm"),
    UnknownLookupField
);
define_nmea_enum!(
    DgnssModeLookup,
    (0, None, "None"),
    (3, Sbas, "SBAS"),
    (1, SbasIfAvailable, "SBAS if available"),
    UnknownLookupField
);
define_nmea_enum!(
    DirectionLookup,
    (0, Forward, "Forward"),
    (1, Reverse, "Reverse"),
    UnknownLookupField
);
define_nmea_enum!(
    DirectionReferenceLookup,
    (2, Error, "Error"),
    (1, Magnetic, "Magnetic"),
    (0, True, "True"),
    UnknownLookupField
);
define_nmea_enum!(
    DirectionRudderLookup,
    (2, MoveToPort, "Move to port"),
    (1, MoveToStarboard, "Move to starboard"),
    (0, NoOrder, "No Order"),
    UnknownLookupField
);
define_nmea_enum!(
    DockingStatusLookup,
    (1, FullyDocked, "Fully docked"),
    (0, NotDocked, "Not docked"),
    UnknownLookupField
);
define_nmea_enum!(
    DscCategoryLookup,
    (112, Distress, "Distress"),
    (100, Routine, "Routine"),
    (108, Safety, "Safety"),
    (110, Urgency, "Urgency"),
    UnknownLookupField
);
define_nmea_enum!(
    DscExpansionDataLookup,
    (
        104,
        AdditionalStationIdentification,
        "Additional station identification"
    ),
    (103, Cog, "COG"),
    (105, EnhancedGeographicArea, "Enhanced geographic area"),
    (100, EnhancedPosition, "Enhanced position"),
    (106, NumberOfPersonsOnBoard, "Number of persons on board"),
    (102, Sog, "SOG"),
    (
        101,
        SourceAndDatumOfPosition,
        "Source and datum of position"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    DscFirstTelecommandLookup,
    (106, Data, "Data"),
    (110, DistressAcknowledgement, "Distress acknowledgement"),
    (112, DistressRelay, "Distress relay"),
    (105, EndOfCall, "End of call"),
    (115, F1Bj2BTtyarq, "F1B/J2B TTY-ARQ"),
    (113, F1Bj2BTtyfec, "F1B/J2B TTY-FEC"),
    (100, F3Eg3EAllModesTp, "F3E/G3E All modes TP"),
    (101, F3Eg3EDuplexTp, "F3E/G3E duplex TP"),
    (109, J3ETp, "J3E TP"),
    (126, NoInformation, "No information"),
    (103, Polling, "Polling"),
    (
        121,
        ShipPositionOrLocationRegistrationUpdating,
        "Ship position or location registration updating"
    ),
    (118, Test, "Test"),
    (104, UnableToComply, "Unable to comply"),
    UnknownLookupField
);
define_nmea_enum!(
    DscFormatLookup,
    (116, AllShips, "All ships"),
    (114, CommonInterest, "Common interest"),
    (112, Distress, "Distress"),
    (102, GeographicalArea, "Geographical area"),
    (
        123,
        IndividualStationAutomatic,
        "Individual station automatic"
    ),
    (120, IndividualStations, "Individual stations"),
    (121, NoncallingPurpose, "Non-calling purpose"),
    UnknownLookupField
);
define_nmea_enum!(
    DscNatureLookup,
    (108, AbandoningShip, "Abandoning ship"),
    (102, Collision, "Collision"),
    (106, DisabledAndAdrift, "Disabled and adrift"),
    (112, EpirbEmission, "EPIRB emission"),
    (100, Fire, "Fire"),
    (101, Flooding, "Flooding"),
    (103, Grounding, "Grounding"),
    (104, Listing, "Listing"),
    (110, ManOverboard, "Man overboard"),
    (109, Piracy, "Piracy"),
    (105, Sinking, "Sinking"),
    (107, Undesignated, "Undesignated"),
    UnknownLookupField
);
define_nmea_enum!(
    DscSecondTelecommandLookup,
    (102, Busy, "Busy"),
    (101, CongestionAtMsc, "Congestion at MSC"),
    (107, EquipmentDisabled, "Equipment disabled"),
    (113, Faxdata, "Fax/data"),
    (111, MedicalTransports, "Medical transports"),
    (126, NoInformation, "No information"),
    (105, NoOperatorAvailable, "No operator available"),
    (100, NoReasonGiven, "No reason given"),
    (
        106,
        OperatorTemporarilyUnavailable,
        "Operator temporarily unavailable"
    ),
    (
        112,
        PayPhonepublicCallOffice,
        "Pay phone/public call office"
    ),
    (103, QueueIndication, "Queue indication"),
    (
        110,
        ShipsAndAircraftOfStatesNotPartiesToAnArmedConflict,
        "Ships and aircraft of States not parties to an armed conflict"
    ),
    (104, StationBarred, "Station barred"),
    (
        108,
        UnableToUseProposedChannel,
        "Unable to use proposed channel"
    ),
    (109, UnableToUseProposedMode, "Unable to use proposed mode"),
    UnknownLookupField
);
define_nmea_enum!(
    EngineInstanceLookup,
    (1, DualEngineStarboard, "Dual Engine Starboard"),
    (
        0,
        SingleEngineOrDualEnginePort,
        "Single Engine or Dual Engine Port"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    EngineStatus1Lookup,
    (9, ChargeIndicator, "Charge Indicator"),
    (0, CheckEngine, "Check Engine"),
    (13, EgrSystem, "EGR System"),
    (15, EmergencyStop, "Emergency Stop"),
    (11, HighBoostPressure, "High Boost Pressure"),
    (6, LowCoolantLevel, "Low Coolant Level"),
    (4, LowFuelPressure, "Low Fuel Pressure"),
    (3, LowOilLevel, "Low Oil Level"),
    (2, LowOilPressure, "Low Oil Pressure"),
    (5, LowSystemVoltage, "Low System Voltage"),
    (1, OverTemperature, "Over Temperature"),
    (10, PreheatIndicator, "Preheat Indicator"),
    (12, RevLimitExceeded, "Rev Limit Exceeded"),
    (14, ThrottlePositionSensor, "Throttle Position Sensor"),
    (7, WaterFlow, "Water Flow"),
    (8, WaterInFuel, "Water In Fuel"),
    UnknownLookupField
);
define_nmea_enum!(
    EngineStatus2Lookup,
    (4, EngineCommError, "Engine Comm Error"),
    (7, EngineShuttingDown, "Engine Shutting Down"),
    (3, MaintenanceNeeded, "Maintenance Needed"),
    (6, NeutralStartProtect, "Neutral Start Protect"),
    (2, PowerReduction, "Power Reduction"),
    (5, SubOrSecondaryThrottle, "Sub or Secondary Throttle"),
    (0, WarningLevel1, "Warning Level 1"),
    (1, WarningLevel2, "Warning Level 2"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentChannelLookup,
    (0, AllChannels, "All channels"),
    (9, BackLeft, "Back left"),
    (10, BackRight, "Back right"),
    (5, Center, "Center"),
    (7, FrontLeft, "Front left"),
    (8, FrontRight, "Front right"),
    (3, StereoBack, "Stereo back"),
    (2, StereoFront, "Stereo front"),
    (1, StereoFullRange, "Stereo full range"),
    (4, StereoSurround, "Stereo surround"),
    (6, Subwoofer, "Subwoofer"),
    (11, SurroundLeft, "Surround left"),
    (12, SurroundRight, "Surround right"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentDefaultSettingsLookup,
    (2, LoadManufacturerDefault, "Load manufacturer default"),
    (1, LoadUserDefault, "Load user default"),
    (
        0,
        SaveCurrentSettingsAsUserDefault,
        "Save current settings as user default"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentEqLookup,
    (8, Arena, "Arena"),
    (9, Cinema, "Cinema"),
    (6, Classic, "Classic"),
    (10, Custom, "Custom"),
    (0, Flat, "Flat"),
    (2, Hall, "Hall"),
    (3, Jazz, "Jazz"),
    (5, Live, "Live"),
    (4, Pop, "Pop"),
    (1, Rock, "Rock"),
    (7, Vocal, "Vocal"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentFilterLookup,
    (3, BandPass, "Band pass"),
    (0, FullRange, "Full range"),
    (1, HighPass, "High pass"),
    (2, LowPass, "Low pass"),
    (4, NotchFilter, "Notch filter"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentGroupLookup,
    (3, AlbumName, "Album Name"),
    (4, ArtistName, "Artist Name"),
    (10, ContentInfo, "Content Info"),
    (8, FavouriteNumber, "Favourite Number"),
    (0, File, "File"),
    (2, GenreName, "Genre Name"),
    (9, PlayQueue, "Play Queue"),
    (1, PlaylistName, "Playlist Name"),
    (6, StationName, "Station Name"),
    (7, StationNumber, "Station Number"),
    (5, TrackName, "Track Name"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentGroupBitfieldLookup,
    (3, AlbumName, "Album Name"),
    (4, ArtistName, "Artist Name"),
    (10, ContentInfo, "Content Info"),
    (8, FavouriteNumber, "Favourite Number"),
    (0, File, "File"),
    (2, GenreName, "Genre Name"),
    (9, PlayQueue, "Play Queue"),
    (1, PlaylistName, "Playlist Name"),
    (6, StationName, "Station Name"),
    (7, StationNumber, "Station Number"),
    (5, TrackName, "Track Name"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentIdTypeLookup,
    (3, EncryptedFile, "Encrypted file"),
    (2, EncryptedGroup, "Encrypted group"),
    (1, File, "File"),
    (0, Group, "Group"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentLikeStatusLookup,
    (0, None, "None"),
    (2, ThumbsDown, "Thumbs down"),
    (1, ThumbsUp, "Thumbs up"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentPlayStatusLookup,
    (3, Ff1X, "FF 1x"),
    (4, Ff2X, "FF 2x"),
    (5, Ff3X, "FF 3x"),
    (6, Ff4X, "FF 4x"),
    (13, JogAhead, "Jog ahead"),
    (14, JogBack, "Jog back"),
    (1, Pause, "Pause"),
    (0, Play, "Play"),
    (7, Rw1X, "RW 1x"),
    (8, Rw2X, "RW 2x"),
    (9, Rw3X, "RW 3x"),
    (10, Rw4X, "RW 4x"),
    (18, ScanDown, "Scan down"),
    (17, ScanUp, "Scan up"),
    (16, SeekDown, "Seek down"),
    (15, SeekUp, "Seek up"),
    (11, SkipAhead, "Skip ahead"),
    (12, SkipBack, "Skip back"),
    (24, SlowMotion125X, "Slow motion .125x"),
    (23, SlowMotion25X, "Slow motion .25x"),
    (22, SlowMotion5X, "Slow motion .5x"),
    (21, SlowMotion75X, "Slow motion .75x"),
    (2, Stop, "Stop"),
    (20, TuneDown, "Tune down"),
    (19, TuneUp, "Tune up"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentPlayStatusBitfieldLookup,
    (3, Ff1X, "FF 1x"),
    (4, Ff2X, "FF 2x"),
    (5, Ff3X, "FF 3x"),
    (6, Ff4X, "FF 4x"),
    (13, JogAhead, "Jog ahead"),
    (14, JogBack, "Jog back"),
    (1, Pause, "Pause"),
    (0, Play, "Play"),
    (7, Rw1X, "RW 1x"),
    (8, Rw2X, "RW 2x"),
    (9, Rw3X, "RW 3x"),
    (10, Rw4X, "RW 4x"),
    (18, ScanDown, "Scan down"),
    (17, ScanUp, "Scan up"),
    (16, SeekDown, "Seek down"),
    (15, SeekUp, "Seek up"),
    (11, SkipAhead, "Skip ahead"),
    (12, SkipBack, "Skip back"),
    (24, SlowMotion125X, "Slow motion .125x"),
    (23, SlowMotion25X, "Slow motion .25x"),
    (22, SlowMotion5X, "Slow motion .5x"),
    (21, SlowMotion75X, "Slow motion .75x"),
    (25, SourceRenaming, "Source renaming"),
    (2, Stop, "Stop"),
    (20, TuneDown, "Tune down"),
    (19, TuneUp, "Tune up"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentRegionsLookup,
    (2, Asia, "Asia"),
    (5, Australia, "Australia"),
    (1, Europe, "Europe"),
    (7, Japan, "Japan"),
    (4, LatinAmerica, "Latin America"),
    (3, MiddleEast, "Middle East"),
    (6, Russia, "Russia"),
    (0, Usa, "USA"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentRepeatBitfieldLookup,
    (1, PlayQueue, "Play queue"),
    (0, Song, "Song"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentRepeatStatusLookup,
    (2, All, "All"),
    (0, Off, "Off"),
    (1, One, "One"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentShuffleBitfieldLookup,
    (1, All, "All"),
    (0, PlayQueue, "Play queue"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentShuffleStatusLookup,
    (2, All, "All"),
    (0, Off, "Off"),
    (1, PlayQueue, "Play queue"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentSourceLookup,
    (1, Am, "AM"),
    (10, Android, "Android"),
    (17, AppleRadio, "Apple Radio"),
    (9, AppleIOs, "Apple iOS"),
    (5, Aux, "Aux"),
    (11, Bluetooth, "Bluetooth"),
    (7, Cd, "CD"),
    (4, Dab, "DAB"),
    (19, Ethernet, "Ethernet"),
    (2, Fm, "FM"),
    (23, Hdmi, "HDMI"),
    (18, LastFm, "Last FM"),
    (8, Mp3, "MP3"),
    (13, Pandora, "Pandora"),
    (12, SiriusXm, "Sirius XM"),
    (15, Slacker, "Slacker"),
    (16, Songza, "Songza"),
    (14, Spotify, "Spotify"),
    (6, Usb, "USB"),
    (0, VesselAlarm, "Vessel alarm"),
    (24, Video, "Video"),
    (22, VideoBluRay, "Video BluRay"),
    (21, VideoDvd, "Video DVD"),
    (20, VideoMp4, "Video MP4"),
    (3, Weather, "Weather"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentTypeLookup,
    (3, AlbumName, "Album Name"),
    (4, ArtistName, "Artist Name"),
    (10, ContentInfo, "Content Info"),
    (8, FavouriteNumber, "Favourite Number"),
    (0, File, "File"),
    (2, GenreName, "Genre Name"),
    (9, PlayQueue, "Play Queue"),
    (1, PlaylistName, "Playlist Name"),
    (6, StationName, "Station Name"),
    (7, StationNumber, "Station Number"),
    (5, TrackName, "Track Name"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentVolumeControlLookup,
    (1, Down, "Down"),
    (0, Up, "Up"),
    UnknownLookupField
);
define_nmea_enum!(
    EntertainmentZoneLookup,
    (0, AllZones, "All zones"),
    (1, Zone1, "Zone 1"),
    (2, Zone2, "Zone 2"),
    (3, Zone3, "Zone 3"),
    (4, Zone4, "Zone 4"),
    UnknownLookupField
);
define_nmea_enum!(
    EquipmentStatusLookup,
    (1, Fault, "Fault"),
    (0, Operational, "Operational"),
    UnknownLookupField
);
define_nmea_enum!(
    FusionCommandLookup,
    (4, Next, "Next"),
    (2, Pause, "Pause"),
    (1, Play, "Play"),
    (6, Prev, "Prev"),
    UnknownLookupField
);
define_nmea_enum!(
    FusionMessageIdLookup,
    (11, AmfmStation, "AM/FM Station"),
    (17, MenuItem, "Menu Item"),
    (23, Mute, "Mute"),
    (32, Power, "Power"),
    (20, Replay, "Replay"),
    (1, RequestStatus, "Request Status"),
    (14, Scan, "Scan"),
    (25, SetAllVolumes, "Set All Volumes"),
    (24, SetZoneVolume, "Set Zone Volume"),
    (38, SiriusXmArtist, "SiriusXM Artist"),
    (36, SiriusXmChannel, "SiriusXM Channel"),
    (40, SiriusXmGenre, "SiriusXM Genre"),
    (37, SiriusXmTitle, "SiriusXM Title"),
    (2, Source, "Source"),
    (13, Squelch, "Squelch"),
    (26, SubVolume, "Sub Volume"),
    (27, Tone, "Tone"),
    (7, TrackAlbum, "Track Album"),
    (6, TrackArtist, "Track Artist"),
    (4, TrackInfo, "Track Info"),
    (9, TrackProgress, "Track Progress"),
    (5, TrackTitle, "Track Title"),
    (33, UnitName, "Unit Name"),
    (12, Vhf, "VHF"),
    (29, Volume, "Volume"),
    (45, ZoneName, "Zone Name"),
    UnknownLookupField
);
define_nmea_enum!(
    FusionMuteCommandLookup,
    (2, MuteOff, "Mute Off"),
    (1, MuteOn, "Mute On"),
    UnknownLookupField
);
define_nmea_enum!(
    FusionPowerStateLookup,
    (2, Off, "Off"),
    (1, On, "On"),
    UnknownLookupField
);
define_nmea_enum!(
    FusionRadioSourceLookup,
    (0, Am, "AM"),
    (1, Fm, "FM"),
    UnknownLookupField
);
define_nmea_enum!(
    FusionReplayModeLookup,
    (9, UsbRepeat, "USB repeat"),
    (10, UsbShuffle, "USB shuffle"),
    (12, IPodRepeat, "iPod repeat"),
    (13, IPodShuffle, "iPod shuffle"),
    UnknownLookupField
);
define_nmea_enum!(
    FusionReplayStatusLookup,
    (2, Allalbum, "All/album"),
    (0, Off, "Off"),
    (1, Onetrack, "One/track"),
    UnknownLookupField
);
define_nmea_enum!(
    FusionSiriusCommandLookup,
    (1, Next, "Next"),
    (2, Prev, "Prev"),
    UnknownLookupField
);
define_nmea_enum!(
    GarminBacklightLevelLookup,
    (0, UnformattableVariantA, "0%"),
    (2, UnformattableVariantB, "10%"),
    (20, UnformattableVariantC, "100%"),
    (3, UnformattableVariantD, "15%"),
    (4, UnformattableVariantE, "20%"),
    (5, UnformattableVariantF, "25%"),
    (6, UnformattableVariantG, "30%"),
    (7, UnformattableVariantH, "35%"),
    (8, UnformattableVariantI, "40%"),
    (9, UnformattableVariantJ, "45%"),
    (1, UnformattableVariantK, "5%"),
    (10, UnformattableVariantL, "50%"),
    (11, UnformattableVariantM, "55%"),
    (12, UnformattableVariantN, "60%"),
    (13, UnformattableVariantO, "65%"),
    (14, UnformattableVariantP, "70%"),
    (15, UnformattableVariantQ, "75%"),
    (16, UnformattableVariantR, "80%"),
    (17, UnformattableVariantS, "85%"),
    (18, UnformattableVariantT, "90%"),
    (19, UnformattableVariantU, "95%"),
    UnknownLookupField
);
define_nmea_enum!(
    GarminColorLookup,
    (0, DayFullColor, "Day full color"),
    (1, DayHighContrast, "Day high contrast"),
    (2, NightFullColor, "Night full color"),
    (4, NightGreenblack, "Night green/black"),
    (3, NightRedblack, "Night red/black"),
    UnknownLookupField
);
define_nmea_enum!(
    GarminColorModeLookup,
    (13, Color, "Color"),
    (0, Day, "Day"),
    (1, Night, "Night"),
    UnknownLookupField
);
define_nmea_enum!(
    GearStatusLookup,
    (0, Forward, "Forward"),
    (1, Neutral, "Neutral"),
    (2, Reverse, "Reverse"),
    UnknownLookupField
);
define_nmea_enum!(
    GnsLookup,
    (5, Chayka, "Chayka"),
    (1, Glonass, "GLONASS"),
    (0, Gps, "GPS"),
    (2, Gpsglonass, "GPS+GLONASS"),
    (3, Gpssbaswaas, "GPS+SBAS/WAAS"),
    (4, Gpssbaswaasglonass, "GPS+SBAS/WAAS+GLONASS"),
    (8, Galileo, "Galileo"),
    (6, Integrated, "integrated"),
    (7, Surveyed, "surveyed"),
    UnknownLookupField
);
define_nmea_enum!(
    GnssModeLookup,
    (0, UnformattableVariantA, "1D"),
    (1, UnformattableVariantB, "2D"),
    (2, UnformattableVariantC, "3D"),
    (3, Auto, "Auto"),
    UnknownLookupField
);
define_nmea_enum!(
    GnsIntegrityLookup,
    (2, Caution, "Caution"),
    (0, NoIntegrityChecking, "No integrity checking"),
    (1, Safe, "Safe"),
    UnknownLookupField
);
define_nmea_enum!(
    GnsMethodLookup,
    (2, DgnssFix, "DGNSS fix"),
    (6, EstimatedDrMode, "Estimated (DR) mode"),
    (1, GnssFix, "GNSS fix"),
    (7, ManualInput, "Manual Input"),
    (3, PreciseGnss, "Precise GNSS"),
    (4, RtkFixedInteger, "RTK Fixed Integer"),
    (5, RtkFloat, "RTK float"),
    (8, SimulateMode, "Simulate mode"),
    (0, NoGnss, "no GNSS"),
    UnknownLookupField
);
define_nmea_enum!(
    GoodWarningErrorLookup,
    (2, Error, "Error"),
    (0, Good, "Good"),
    (1, Warning, "Warning"),
    UnknownLookupField
);
define_nmea_enum!(
    GroupFunctionLookup,
    (2, Acknowledge, "Acknowledge"),
    (1, Command, "Command"),
    (3, ReadFields, "Read Fields"),
    (4, ReadFieldsReply, "Read Fields Reply"),
    (0, Request, "Request"),
    (5, WriteFields, "Write Fields"),
    (6, WriteFieldsReply, "Write Fields Reply"),
    UnknownLookupField
);
define_nmea_enum!(
    HumiditySourceLookup,
    (0, Inside, "Inside"),
    (1, Outside, "Outside"),
    UnknownLookupField
);
define_nmea_enum!(
    IndustryCodeLookup,
    (2, Agriculture, "Agriculture"),
    (3, Construction, "Construction"),
    (0, Global, "Global"),
    (1, Highway, "Highway"),
    (5, Industrial, "Industrial"),
    (4, Marine, "Marine"),
    UnknownLookupField
);
define_nmea_enum!(
    InverterStateLookup,
    (1, AcPassthru, "AC passthru"),
    (4, Disabled, "Disabled"),
    (3, Fault, "Fault"),
    (0, Invert, "Invert"),
    (2, LoadSense, "Load sense"),
    UnknownLookupField
);
define_nmea_enum!(
    IsoCommandLookup,
    (0, Ack, "ACK"),
    (255, Abort, "Abort"),
    (32, Bam, "BAM"),
    (17, Cts, "CTS"),
    (19, Eom, "EOM"),
    (16, Rts, "RTS"),
    UnknownLookupField
);
define_nmea_enum!(
    IsoControlLookup,
    (0, Ack, "ACK"),
    (2, AccessDenied, "Access Denied"),
    (3, AddressBusy, "Address Busy"),
    (1, Nak, "NAK"),
    UnknownLookupField
);
define_nmea_enum!(
    LightingCommandLookup,
    (1, DetectDevices, "Detect Devices"),
    (3, FactoryReset, "Factory Reset"),
    (0, Idle, "Idle"),
    (4, PoweringUp, "Powering Up"),
    (2, Reboot, "Reboot"),
    UnknownLookupField
);
define_nmea_enum!(
    LineLookup,
    (0, Line1, "Line 1"),
    (1, Line2, "Line 2"),
    (2, Line3, "Line 3"),
    UnknownLookupField
);
define_nmea_enum!(
    LowBatteryLookup,
    (0, Good, "Good"),
    (1, Low, "Low"),
    UnknownLookupField
);
define_nmea_enum!(
    MagneticVariationLookup,
    (3, AutomaticCalculation, "Automatic Calculation"),
    (1, AutomaticChart, "Automatic Chart"),
    (2, AutomaticTable, "Automatic Table"),
    (0, Manual, "Manual"),
    (4, Wmm2000, "WMM 2000"),
    (5, Wmm2005, "WMM 2005"),
    (6, Wmm2010, "WMM 2010"),
    (7, Wmm2015, "WMM 2015"),
    (8, Wmm2020, "WMM 2020"),
    UnknownLookupField
);
define_nmea_enum!(
    ManufacturerCodeLookup,
    (69, ArksEnterprisesInc, "ARKS Enterprises, Inc."),
    (905, AsaElectronics, "ASA Electronics"),
    (199, Actia, "Actia"),
    (273, Actisense, "Actisense"),
    (578, Advansea, "Advansea"),
    (135, Airmar, "Airmar"),
    (
        459,
        AlltekMarineElectronicsCorp,
        "Alltek Marine Electronics Corp"
    ),
    (274, AmphenolLtwTechnology, "Amphenol LTW Technology"),
    (600, AquaticAv, "Aquatic AV"),
    (614, ArltTecnologies, "Arlt Tecnologies"),
    (502, AttwoodMarine, "Attwood Marine"),
    (735, AuElectronicsGroup, "Au Electronics Group"),
    (715, Autonnic, "Autonnic"),
    (605, AventicsGmbH, "Aventics GmbH"),
    (381, BG, "B & G"),
    (295, BepMarine, "BEP Marine"),
    (802, BjTechnologiesBeneteau, "BJ Technologies (Beneteau)"),
    (637, BavariaYacts, "Bavaria Yacts"),
    (185, BeedeInstruments, "Beede Instruments"),
    (396, BeyondMeasure, "Beyond Measure"),
    (969, BlueSeas, "Blue Seas"),
    (148, BlueWaterData, "Blue Water Data"),
    (811, BlueWaterDesalination, "Blue Water Desalination"),
    (795, BroydaIndustries, "Broyda Industries"),
    (
        341,
        BöningAutomationstechnologieGmbHCoKg,
        "Böning Automationstechnologie GmbH & Co. KG"
    ),
    (165, CpacSystemsAb, "CPAC Systems AB"),
    (796, CanadianAutomotive, "Canadian Automotive"),
    (394, Capi2, "Capi 2"),
    (
        176,
        CarlingTechnologiesIncMoritzAerospace,
        "Carling Technologies Inc. (Moritz Aerospace)"
    ),
    (409, Chetco, "Chetco"),
    (
        481,
        ChetcoDigitialInstruments,
        "Chetco Digitial Instruments"
    ),
    (773, ClarionUs, "Clarion US"),
    (286, CoelmoSrlItaly, "Coelmo SRL Italy"),
    (404, ComNav, "ComNav"),
    (438, ComarSystemsLimited, "Comar Systems Limited"),
    (968, CoxPowertrain, "Cox Powertrain"),
    (440, Cummins, "Cummins"),
    (743, DaeMyung, "DaeMyung"),
    (868, DataPanelCorp, "Data Panel Corp"),
    (329, Dief, "Dief"),
    (211, DigitalSwitchingSystems, "Digital Switching Systems"),
    (437, DigitalYacht, "Digital Yacht"),
    (201, DisenosYTechnological, "Disenos Y Technological"),
    (641, DiverseYachtServices, "Diverse Yacht Services"),
    (224, EmmiNetworkSl, "EMMI NETWORK S.L."),
    (930, Ecotronix, "Ecotronix"),
    (
        426,
        EgersundMarineElectronicsAs,
        "Egersund Marine Electronics AS"
    ),
    (373, ElectronicDesign, "Electronic Design"),
    (304, EmpirBus, "Empir Bus"),
    (243, Eride, "Eride"),
    (163, EvinrudeBrp, "Evinrude/BRP"),
    (815, Flir, "FLIR"),
    (
        78,
        FwMurphyEnovationControls,
        "FW Murphy/Enovation Controls"
    ),
    (1863, FariaInstruments, "Faria Instruments"),
    (844, FellMarine, "Fell Marine"),
    (311, FischerPanda, "Fischer Panda"),
    (785, FischerPandaDe, "Fischer Panda DE"),
    (356, FischerPandaGenerators, "Fischer Panda Generators"),
    (192, FloscanInstrumentCoInc, "Floscan Instrument Co. Inc."),
    (1855, Furuno, "Furuno"),
    (419, FusionElectronics, "Fusion Electronics"),
    (
        475,
        GmeAkaStandardCommunicationsPtyLtd,
        "GME aka Standard Communications Pty LTD"
    ),
    (645, Garmin, "Garmin"),
    (803, GillSensors, "Gill Sensors"),
    (378, Glendinning, "Glendinning"),
    (272, Groco, "Groco"),
    (776, HmiSystems, "HMI Systems"),
    (283, HamiltonJet, "Hamilton Jet"),
    (88, HemisphereGpsInc, "Hemisphere GPS Inc"),
    (250, HondaMarine, "Honda Marine"),
    (257, HondaMotorCompanyLtd, "Honda Motor Company LTD"),
    (
        476,
        HumminbirdMarineElectronics,
        "Humminbird Marine Electronics"
    ),
    (315, Icom, "ICOM"),
    (606, Intellian, "Intellian"),
    (704, JlAudio, "JL Audio"),
    (1853, JapanRadioCo, "Japan Radio Co"),
    (
        385,
        JohnsonOutdoorsMarineElectronicsIncGeonav,
        "Johnson Outdoors Marine Electronics Inc Geonav"
    ),
    (579, Kvh, "KVH"),
    (85, KohlerPowerSystems, "Kohler Power Systems"),
    (345, KoreanMaritimeUniversity, "Korean Maritime University"),
    (1859, KvasarAb, "Kvasar AB"),
    (890, L3Technologies, "L3 Technologies"),
    (499, LcjCaptures, "Lcj Captures"),
    (1858, Litton, "Litton"),
    (400, LivorsiMarine, "Livorsi Marine"),
    (140, Lowrance, "Lowrance"),
    (798, Lumishore, "Lumishore"),
    (739, LxNav, "LxNav"),
    (307, MbwTechnologies, "MBW Technologies"),
    (1860, Mmp, "MMP"),
    (137, Maretron, "Maretron"),
    (571, MarinecraftSouthKorea, "Marinecraft (South Korea)"),
    (909, MarinesCoSouthKorea, "Marines Co (South Korea)"),
    (510, MarinesoftCoLtd, "Marinesoft Co. LTD"),
    (355, Mastervolt, "Mastervolt"),
    (
        573,
        McMurdoGroupAkaOroliaLtd,
        "McMurdo Group aka Orolia LTD"
    ),
    (144, MercuryMarine, "Mercury Marine"),
    (
        198,
        MysticValleyCommunications,
        "Mystic Valley Communications"
    ),
    (529, NationalInstrumentsKorea, "National Instruments Korea"),
    (147, NautibusElectronicGmbH, "Nautibus Electronic GmbH"),
    (911, Nauticon, "Nautic-on"),
    (275, Navico, "Navico"),
    (1852, Navionics, "Navionics"),
    (503, NaviopSrl, "Naviop S.R.L."),
    (896, NexfourSolutions, "Nexfour Solutions"),
    (517, NoLandEngineering, "NoLand Engineering"),
    (193, Nobletec, "Nobletec"),
    (374, NorthernLights, "Northern Lights"),
    (1854, NorthstarTechnologies, "Northstar Technologies"),
    (305, NovAtel, "NovAtel"),
    (478, OceanSatBv, "Ocean Sat BV"),
    (777, OceanSignal, "Ocean Signal"),
    (847, Oceanvolt, "Oceanvolt"),
    (161, OffshoreSystemsUkLtd, "Offshore Systems (UK) Ltd."),
    (532, OnwaMarine, "Onwa Marine"),
    (
        451,
        ParkerHannifinAkaVillageMarineTech,
        "Parker Hannifin aka Village Marine Tech"
    ),
    (781, PolyPlanar, "Poly Planar"),
    (862, Prospec, "Prospec"),
    (328, Qwerty, "Qwerty"),
    (734, ReapSystems, "REAP Systems"),
    (1851, Raymarine, "Raymarine"),
    (894, RhodanMarineSystems, "Rhodan Marine Systems"),
    (688, RockfordCorp, "Rockford Corp"),
    (370, RollsRoyceMarine, "Rolls Royce Marine"),
    (
        384,
        RosePointNavigationSystems,
        "Rose Point Navigation Systems"
    ),
    (460, SanGiorgioSein, "SAN GIORGIO S.E.I.N"),
    (470, SitexMarineElectronics, "SI-TEX Marine Electronics"),
    (
        235,
        SailormadeMarineTelemetryTetraTechnologyLtd,
        "Sailormade Marine Telemetry/Tetra Technology LTD"
    ),
    (612, SamwonIt, "SamwonIT"),
    (580, SanJoseTechnology, "San Jose Technology"),
    (471, SeaCrossMarineAb, "Sea Cross Marine AB"),
    (285, SeaRecovery, "Sea Recovery"),
    (778, Seekeeper, "Seekeeper"),
    (
        658,
        ShenzhenJiuzhouHimunication,
        "Shenzhen Jiuzhou Himunication"
    ),
    (595, ShipModuleAkaCustomware, "Ship Module aka Customware"),
    (1857, Simrad, "Simrad"),
    (306, SleipnerMotorAs, "Sleipner Motor AS"),
    (421, StandardHorizon, "Standard Horizon"),
    (
        799,
        StillWaterDesignsAndAudio,
        "Still Water Designs and Audio"
    ),
    (586, SuzukiMotorCorporation, "Suzuki Motor Corporation"),
    (963, TjcMicro, "TJC Micro"),
    (838, TeamSurv, "TeamSurv"),
    (
        1850,
        TeleflexMarineSeaStarSolutions,
        "Teleflex Marine (SeaStar Solutions)"
    ),
    (351, ThraneAndThrane, "Thrane and Thrane"),
    (797, TidesMarine, "Tides Marine"),
    (962, TimbolierIndustries, "Timbolier Industries"),
    (431, TohatsuCoJp, "Tohatsu Co, JP"),
    (518, TransasUsa, "Transas USA"),
    (1856, Trimble, "Trimble"),
    (422, TrueHeadingAb, "True Heading AB"),
    (80, TwinDisc, "Twin Disc"),
    (591, UsCoastGuard, "US Coast Guard"),
    (824, UndheimSystems, "Undheim Systems"),
    (
        443,
        VdoAkaContinentalCorporation,
        "VDO (aka Continental-Corporation)"
    ),
    (1861, VectorCantech, "Vector Cantech"),
    (
        466,
        VeethreeElectronicsMarine,
        "Veethree Electronics & Marine"
    ),
    (504, VesperMarineLtd, "Vesper Marine Ltd"),
    (358, VictronEnergy, "Victron Energy"),
    (174, VolvoPenta, "Volvo Penta"),
    (493, Watcheye, "Watcheye"),
    (644, WemaUsaDbaKus, "Wema U.S.A dba KUS"),
    (154, Westerbeke, "Westerbeke"),
    (744, Woosung, "Woosung"),
    (168, XantrexTechnologyInc, "Xantrex Technology Inc."),
    (215, XintexAtena, "Xintex/Atena"),
    (583, YachtControl, "Yacht Control"),
    (717, YachtDevices, "Yacht Devices"),
    (233, YachtMonitoringSolutions, "Yacht Monitoring Solutions"),
    (1862, YamahaMarine, "Yamaha Marine"),
    (172, YanmarMarine, "Yanmar Marine"),
    (228, Zf, "ZF"),
    (427, EmtrakMarineElectronics, "em-trak Marine Electronics"),
    UnknownLookupField
);
define_nmea_enum!(
    MarkTypeLookup,
    (0, Collision, "Collision"),
    (2, Reference, "Reference"),
    (1, TurningPoint, "Turning point"),
    (4, Waypoint, "Waypoint"),
    (3, Wheelover, "Wheelover"),
    UnknownLookupField
);
define_nmea_enum!(
    MobPositionSourceLookup,
    (
        0,
        PositionEstimatedByTheVessel,
        "Position estimated by the vessel"
    ),
    (
        1,
        PositionReportedByMobEmitter,
        "Position reported by MOB emitter"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    MobStatusLookup,
    (0, MobEmitterActivated, "MOB Emitter Activated"),
    (
        1,
        ManualOnboardMobButtonActivation,
        "Manual on-board MOB Button Activation"
    ),
    (2, TestMode, "Test mode"),
    UnknownLookupField
);
define_nmea_enum!(
    NavStatusLookup,
    (14, Aissart, "AIS-SART"),
    (6, Aground, "Aground"),
    (1, AtAnchor, "At anchor"),
    (4, ConstrainedByHerDraught, "Constrained by her draught"),
    (7, EngagedInFishing, "Engaged in Fishing"),
    (
        9,
        HazardousMaterialHighSpeed,
        "Hazardous material - High Speed"
    ),
    (
        10,
        HazardousMaterialWingInGround,
        "Hazardous material - Wing in Ground"
    ),
    (5, Moored, "Moored"),
    (2, NotUnderCommand, "Not under command"),
    (
        12,
        PowerdrivenVesslPushingAheadOrTowingAlongside,
        "Power-driven vessl pushing ahead or towing alongside"
    ),
    (
        11,
        PowerdrivenVesslTowingAstern,
        "Power-driven vessl towing astern"
    ),
    (3, RestrictedManeuverability, "Restricted maneuverability"),
    (8, UnderWaySailing, "Under way sailing"),
    (0, UnderWayUsingEngine, "Under way using engine"),
    UnknownLookupField
);
define_nmea_enum!(
    OffOnLookup,
    (0, Off, "Off"),
    (1, On, "On"),
    UnknownLookupField
);
define_nmea_enum!(
    OkWarningLookup,
    (0, Ok, "OK"),
    (1, Warning, "Warning"),
    UnknownLookupField
);
define_nmea_enum!(
    ParameterFieldLookup,
    (4, AccessDenied, "Access denied"),
    (0, Acknowledge, "Acknowledge"),
    (1, InvalidParameterField, "Invalid parameter field"),
    (5, NotSupported, "Not supported"),
    (3, ParameterOutOfRange, "Parameter out of range"),
    (6, ReadOrWriteNotSupported, "Read or Write not supported"),
    (2, TemporaryError, "Temporary error"),
    UnknownLookupField
);
define_nmea_enum!(
    PgnErrorCodeLookup,
    (3, AccessDenied, "Access denied"),
    (0, Acknowledge, "Acknowledge"),
    (4, NotSupported, "Not supported"),
    (2, PgnNotAvailable, "PGN not available"),
    (1, PgnNotSupported, "PGN not supported"),
    (6, ReadOrWriteNotSupported, "Read or Write not supported"),
    (5, TagNotSupported, "Tag not supported"),
    UnknownLookupField
);
define_nmea_enum!(
    PgnListFunctionLookup,
    (1, ReceivePgnList, "Receive PGN list"),
    (0, TransmitPgnList, "Transmit PGN list"),
    UnknownLookupField
);
define_nmea_enum!(
    PositionAccuracyLookup,
    (1, High, "High"),
    (0, Low, "Low"),
    UnknownLookupField
);
define_nmea_enum!(
    PositionFixDeviceLookup,
    (5, Chayka, "Chayka"),
    (3, CombinedGpsglonass, "Combined GPS/GLONASS"),
    (0, DefaultUndefined, "Default: undefined"),
    (2, Glonass, "GLONASS"),
    (1, Gps, "GPS"),
    (8, Galileo, "Galileo"),
    (
        6,
        IntegratedNavigationSystem,
        "Integrated navigation system"
    ),
    (15, InternalGnss, "Internal GNSS"),
    (4, LoranC, "Loran-C"),
    (7, Surveyed, "Surveyed"),
    UnknownLookupField
);
define_nmea_enum!(
    PowerFactorLookup,
    (2, Error, "Error"),
    (1, Lagging, "Lagging"),
    (0, Leading, "Leading"),
    UnknownLookupField
);
define_nmea_enum!(
    PressureSourceLookup,
    (6, AltimeterSetting, "AltimeterSetting"),
    (0, Atmospheric, "Atmospheric"),
    (3, CompressedAir, "Compressed Air"),
    (5, Filter, "Filter"),
    (8, Fuel, "Fuel"),
    (4, Hydraulic, "Hydraulic"),
    (7, Oil, "Oil"),
    (2, Steam, "Steam"),
    (1, Water, "Water"),
    UnknownLookupField
);
define_nmea_enum!(
    PriorityLookup,
    (0, UnformattableVariantA, "0"),
    (1, UnformattableVariantB, "1"),
    (2, UnformattableVariantC, "2"),
    (3, UnformattableVariantD, "3"),
    (4, UnformattableVariantE, "4"),
    (5, UnformattableVariantF, "5"),
    (6, UnformattableVariantG, "6"),
    (7, UnformattableVariantH, "7"),
    (8, LeaveUnchanged, "Leave unchanged"),
    (9, ResetToDefault, "Reset to default"),
    UnknownLookupField
);
define_nmea_enum!(
    RaimFlagLookup,
    (1, InUse, "in use"),
    (0, NotInUse, "not in use"),
    UnknownLookupField
);
define_nmea_enum!(
    RangeResidualModeLookup,
    (
        1,
        RangeResidualsWereCalculatedAfterThePosition,
        "Range residuals were calculated after the position"
    ),
    (
        0,
        RangeResidualsWereUsedToCalculateData,
        "Range residuals were used to calculate data"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    RepeatIndicatorLookup,
    (3, FinalRetransmission, "Final retransmission"),
    (1, FirstRetransmission, "First retransmission"),
    (0, Initial, "Initial"),
    (2, SecondRetransmission, "Second retransmission"),
    UnknownLookupField
);
define_nmea_enum!(
    ReportingIntervalLookup,
    (4, UnformattableVariantA, "1 min"),
    (1, UnformattableVariantB, "10 min"),
    (7, UnformattableVariantC, "10 sec"),
    (6, UnformattableVariantD, "15 sec"),
    (
        9,
        UnformattableVariantE,
        "2 sec (not applicable to Class B CS)"
    ),
    (3, UnformattableVariantF, "3 min"),
    (5, UnformattableVariantG, "30 sec"),
    (8, UnformattableVariantH, "5 sec"),
    (2, UnformattableVariantI, "6 min"),
    (
        0,
        AsGivenByTheAutonomousMode,
        "As given by the autonomous mode"
    ),
    (
        11,
        NextLongerReportingInterval,
        "Next longer reporting interval"
    ),
    (
        10,
        NextShorterReportingInterval,
        "Next shorter reporting interval"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    ResidualModeLookup,
    (0, Autonomous, "Autonomous"),
    (1, DifferentialEnhanced, "Differential enhanced"),
    (2, Estimated, "Estimated"),
    (4, Manual, "Manual"),
    (3, Simulator, "Simulator"),
    UnknownLookupField
);
define_nmea_enum!(
    RodeTypeLookup,
    (0, ChainPresentlyDetected, "Chain presently detected"),
    (1, RopePresentlyDetected, "Rope presently detected"),
    UnknownLookupField
);
define_nmea_enum!(
    SatelliteStatusLookup,
    (0, NotTracked, "Not tracked"),
    (3, NotTrackedDiff, "Not tracked+Diff"),
    (1, Tracked, "Tracked"),
    (4, TrackedDiff, "Tracked+Diff"),
    (2, Used, "Used"),
    (5, UsedDiff, "Used+Diff"),
    UnknownLookupField
);
define_nmea_enum!(
    SeatalkAlarmGroupLookup,
    (4, Ais, "AIS"),
    (1, Autopilot, "Autopilot"),
    (3, ChartPlotter, "Chart Plotter"),
    (0, Instrument, "Instrument"),
    (2, Radar, "Radar"),
    UnknownLookupField
);
define_nmea_enum!(
    SeatalkAlarmIdLookup,
    (88, Ais12VAlarm, "AIS 12V alarm"),
    (93, Ais3V3Alarm, "AIS 3V3 alarm"),
    (89, Ais6VAlarm, "AIS 6V alarm"),
    (82, AisAntennaVswrFault, "AIS Antenna VSWR fault"),
    (107, AisConnectionLost, "AIS Connection Lost"),
    (104, AisDangerousTarget, "AIS Dangerous Target"),
    (95, AisHeadingLostinvalid, "AIS Heading lost/invalid"),
    (99, AisInternalGgaTimeout, "AIS Internal GGA timeout"),
    (98, AisLockFailure, "AIS Lock failure"),
    (105, AisLostTarget, "AIS Lost Target"),
    (97, AisNoSensorPosition, "AIS No sensor position"),
    (
        85,
        AisNoSensorPositionInUse,
        "AIS No sensor position in use"
    ),
    (87, AisNoValidCogInformation, "AIS No valid COG information"),
    (86, AisNoValidSogInformation, "AIS No valid SOG information"),
    (
        90,
        AisNoiseThresholdExceededChannelA,
        "AIS Noise threshold exceeded channel A"
    ),
    (
        91,
        AisNoiseThresholdExceededChannelB,
        "AIS Noise threshold exceeded channel B"
    ),
    (100, AisProtocolStackRestart, "AIS Protocol stack restart"),
    (83, AisRxChannel1Malfunction, "AIS Rx channel 1 malfunction"),
    (84, AisRxChannel2Malfunction, "AIS Rx channel 2 malfunction"),
    (
        94,
        AisRxChannel70Malfunction,
        "AIS Rx channel 70 malfunction"
    ),
    (
        106,
        AisSafetyRelatedMessageUsedToSilence,
        "AIS Safety Related Message (used to silence)"
    ),
    (81, AisTxMalfunction, "AIS TX Malfunction"),
    (92, AisTransmitterPaFault, "AIS Transmitter PA fault"),
    (96, AisInternalGpsLost, "AIS internal GPS lost"),
    (6, AwaHigh, "AWA High"),
    (7, AwaLow, "AWA Low"),
    (8, AwsHigh, "AWS High"),
    (9, AwsLow, "AWS Low"),
    (15, BoatSpeedHigh, "Boat Speed High"),
    (16, BoatSpeedLow, "Boat Speed Low"),
    (4, DeepAnchor, "Deep Anchor"),
    (2, DeepDepth, "Deep Depth"),
    (37, GpsFailure, "GPS Failure"),
    (38, Mob, "MOB"),
    (0, NoAlarm, "No Alarm"),
    (108, NoFix, "No Fix"),
    (5, OffCourse, "Off Course"),
    (56, PilotAutoDocksideFail, "Pilot Auto Dockside Fail"),
    (28, PilotAutoRelease, "Pilot Auto Release"),
    (62, PilotAutolearnFail1, "Pilot Autolearn Fail1"),
    (63, PilotAutolearnFail2, "Pilot Autolearn Fail2"),
    (64, PilotAutolearnFail3, "Pilot Autolearn Fail3"),
    (65, PilotAutolearnFail4, "Pilot Autolearn Fail4"),
    (66, PilotAutolearnFail5, "Pilot Autolearn Fail5"),
    (67, PilotAutolearnFail6, "Pilot Autolearn Fail6"),
    (27, PilotCuDisconnected, "Pilot CU Disconnected"),
    (32, PilotCalibrationRequired, "Pilot Calibration Required"),
    (48, PilotCurrentLimit, "Pilot Current Limit"),
    (30, PilotDriveStopped, "Pilot Drive Stopped"),
    (60, PilotEepromCorrupt, "Pilot EEPROM Corrupt"),
    (80, PilotInvalidCommand, "Pilot Invalid Command"),
    (75, PilotJoystickFault, "Pilot Joystick Fault"),
    (25, PilotLargeXte, "Pilot Large XTE"),
    (33, PilotLastHeading, "Pilot Last Heading"),
    (23, PilotLastMinuteOfWatch, "Pilot Last Minute Of Watch"),
    (59, PilotLostWaypointData, "Pilot Lost Waypoint Data"),
    (22, PilotLowBattery, "Pilot Low Battery"),
    (26, PilotNmeaDataError, "Pilot NMEA DataError"),
    (46, PilotNoCompass, "Pilot No Compass"),
    (43, PilotNoGpsCog, "Pilot No GPS COG"),
    (42, PilotNoGpsFix, "Pilot No GPS Fix"),
    (101, PilotNoIpsCommunications, "Pilot No IPS communications"),
    (76, PilotNoJoystickData, "Pilot No Joystick Data"),
    (24, PilotNoNmeaData, "Pilot No NMEA Data"),
    (58, PilotNoNavData, "Pilot No Nav Data"),
    (34, PilotNoPilot, "Pilot No Pilot"),
    (52, PilotNoSpeedData, "Pilot No Speed Data"),
    (51, PilotNoWindData, "Pilot No Wind Data"),
    (20, PilotOffCourse, "Pilot Off Course"),
    (
        102,
        PilotPowerOnOrSleepSwitchResetWhileEngaged,
        "Pilot Power-On or Sleep-Switch Reset While Engaged"
    ),
    (47, PilotRateGyroFault, "Pilot Rate Gyro Fault"),
    (35, PilotRouteComplete, "Pilot Route Complete"),
    (61, PilotRudderFeedbackFail, "Pilot Rudder Feedback Fail"),
    (53, PilotSeatalkFail1, "Pilot Seatalk Fail1"),
    (54, PilotSeatalkFail2, "Pilot Seatalk Fail2"),
    (
        41,
        PilotStandbyTooFastToFish,
        "Pilot Standby Too Fast To Fish"
    ),
    (44, PilotStartUp, "Pilot Start Up"),
    (40, PilotSwappedMotorPower, "Pilot Swapped Motor Power"),
    (45, PilotTooSlow, "Pilot Too Slow"),
    (57, PilotTurnTooFast, "Pilot Turn Too Fast"),
    (31, PilotTypeUnspecified, "Pilot Type Unspecified"),
    (
        103,
        PilotUnexpectedResetWhileEngaged,
        "Pilot Unexpected Reset While Engaged"
    ),
    (36, PilotVariableText, "Pilot Variable Text"),
    (68, PilotWarningCalRequired, "Pilot Warning Cal Required"),
    (73, PilotWarningClutchShort, "Pilot Warning Clutch Short"),
    (72, PilotWarningDriveShort, "Pilot Warning Drive Short"),
    (69, PilotWarningOffCourse, "Pilot Warning OffCourse"),
    (
        74,
        PilotWarningSolenoidShort,
        "Pilot Warning Solenoid Short"
    ),
    (
        55,
        PilotWarningTooFastToFish,
        "Pilot Warning Too Fast To Fish"
    ),
    (71, PilotWarningWindShift, "Pilot Warning Wind Shift"),
    (70, PilotWarningXte, "Pilot Warning XTE"),
    (19, PilotWatch, "Pilot Watch"),
    (29, PilotWayPointAdvance, "Pilot Way Point Advance"),
    (49, PilotWayPointAdvancePort, "Pilot Way Point Advance Port"),
    (50, PilotWayPointAdvanceStbd, "Pilot Way Point Advance Stbd"),
    (21, PilotWindShift, "Pilot Wind Shift"),
    (17, SeaTemperatureHigh, "Sea Temperature High"),
    (18, SeaTemperatureLow, "Sea Temperature Low"),
    (39, Seatalk1Anchor, "Seatalk1 Anchor"),
    (3, ShallowAnchor, "Shallow Anchor"),
    (1, ShallowDepth, "Shallow Depth"),
    (10, TwaHigh, "TWA High"),
    (11, TwaLow, "TWA Low"),
    (12, TwsHigh, "TWS High"),
    (13, TwsLow, "TWS Low"),
    (14, WpArrival, "WP Arrival"),
    UnknownLookupField
);
define_nmea_enum!(
    SeatalkAlarmStatusLookup,
    (
        1,
        AlarmConditionMetAndNotSilenced,
        "Alarm condition met and not silenced"
    ),
    (
        2,
        AlarmConditionMetAndSilenced,
        "Alarm condition met and silenced"
    ),
    (0, AlarmConditionNotMet, "Alarm condition not met"),
    UnknownLookupField
);
define_nmea_enum!(
    SeatalkDeviceIdLookup,
    (5, CourseComputer, "Course Computer"),
    (3, S100, "S100"),
    UnknownLookupField
);
define_nmea_enum!(
    SeatalkDisplayColorLookup,
    (0, Day1, "Day 1"),
    (2, Day2, "Day 2"),
    (4, Inverse, "Inverse"),
    (3, RedBlack, "Red/Black"),
    UnknownLookupField
);
define_nmea_enum!(
    SeatalkKeystrokeLookup,
    (7, UnformattableVariantA, "+1"),
    (34, UnformattableVariantB, "+1 and +10"),
    (8, UnformattableVariantC, "+10"),
    (5, UnformattableVariantD, "-1"),
    (33, UnformattableVariantE, "-1 and -10"),
    (6, UnformattableVariantF, "-10"),
    (1, Auto, "Auto"),
    (2, Standby, "Standby"),
    (35, Track, "Track"),
    (3, Wind, "Wind"),
    UnknownLookupField
);
define_nmea_enum!(
    SeatalkNetworkGroupLookup,
    (3, Cockpit, "Cockpit"),
    (4, Flybridge, "Flybridge"),
    (6, Group1, "Group 1"),
    (7, Group2, "Group 2"),
    (8, Group3, "Group 3"),
    (9, Group4, "Group 4"),
    (10, Group5, "Group 5"),
    (1, Helm1, "Helm 1"),
    (2, Helm2, "Helm 2"),
    (5, Mast, "Mast"),
    (0, None, "None"),
    UnknownLookupField
);
define_nmea_enum!(
    SeatalkPilotModeLookup,
    (66, Auto, "Auto"),
    (64, Standby, "Standby"),
    (74, Track, "Track"),
    (70, Wind, "Wind"),
    UnknownLookupField
);
define_nmea_enum!(
    ShipTypeLookup,
    (54, Antipollution, "Anti-pollution"),
    (70, CargoShip, "Cargo ship"),
    (74, CargoShipHazardCatOs, "Cargo ship (hazard cat OS)"),
    (71, CargoShipHazardCatX, "Cargo ship (hazard cat X)"),
    (72, CargoShipHazardCatY, "Cargo ship (hazard cat Y)"),
    (73, CargoShipHazardCatZ, "Cargo ship (hazard cat Z)"),
    (
        79,
        CargoShipNoAdditionalInformation,
        "Cargo ship (no additional information)"
    ),
    (
        34,
        EngagedInDivingOperations,
        "Engaged in diving operations"
    ),
    (
        33,
        EngagedInDredgingOrUnderwaterOperations,
        "Engaged in dredging or underwater operations"
    ),
    (
        35,
        EngagedInMilitaryOperations,
        "Engaged in military operations"
    ),
    (30, Fishing, "Fishing"),
    (40, HighSpeedCraft, "High speed craft"),
    (
        44,
        HighSpeedCraftHazardCatOs,
        "High speed craft (hazard cat OS)"
    ),
    (
        41,
        HighSpeedCraftHazardCatX,
        "High speed craft (hazard cat X)"
    ),
    (
        42,
        HighSpeedCraftHazardCatY,
        "High speed craft (hazard cat Y)"
    ),
    (
        43,
        HighSpeedCraftHazardCatZ,
        "High speed craft (hazard cat Z)"
    ),
    (
        49,
        HighSpeedCraftNoAdditionalInformation,
        "High speed craft (no additional information)"
    ),
    (55, LawEnforcement, "Law enforcement"),
    (58, Medical, "Medical"),
    (90, Other, "Other"),
    (94, OtherHazardCatOs, "Other (hazard cat OS)"),
    (91, OtherHazardCatX, "Other (hazard cat X)"),
    (92, OtherHazardCatY, "Other (hazard cat Y)"),
    (93, OtherHazardCatZ, "Other (hazard cat Z)"),
    (
        99,
        OtherNoAdditionalInformation,
        "Other (no additional information)"
    ),
    (60, PassengerShip, "Passenger ship"),
    (
        64,
        PassengerShipHazardCatOs,
        "Passenger ship (hazard cat OS)"
    ),
    (61, PassengerShipHazardCatX, "Passenger ship (hazard cat X)"),
    (62, PassengerShipHazardCatY, "Passenger ship (hazard cat Y)"),
    (63, PassengerShipHazardCatZ, "Passenger ship (hazard cat Z)"),
    (
        69,
        PassengerShipNoAdditionalInformation,
        "Passenger ship (no additional information)"
    ),
    (50, PilotVessel, "Pilot vessel"),
    (37, Pleasure, "Pleasure"),
    (53, PortTender, "Port tender"),
    (51, Sar, "SAR"),
    (36, Sailing, "Sailing"),
    (
        59,
        ShipsAndAircraftOfStatesNotPartiesToAnArmedConflict,
        "Ships and aircraft of States not parties to an armed conflict"
    ),
    (56, Spare, "Spare"),
    (57, Spare2, "Spare #2"),
    (80, Tanker, "Tanker"),
    (84, TankerHazardCatOs, "Tanker (hazard cat OS)"),
    (81, TankerHazardCatX, "Tanker (hazard cat X)"),
    (82, TankerHazardCatY, "Tanker (hazard cat Y)"),
    (83, TankerHazardCatZ, "Tanker (hazard cat Z)"),
    (
        89,
        TankerNoAdditionalInformation,
        "Tanker (no additional information)"
    ),
    (31, Towing, "Towing"),
    (
        32,
        TowingExceeds200MOrWiderThan25M,
        "Towing exceeds 200m or wider than 25m"
    ),
    (52, Tug, "Tug"),
    (0, Unavailable, "Unavailable"),
    (20, WingInGround, "Wing In Ground"),
    (
        24,
        WingInGroundHazardCatOs,
        "Wing In Ground (hazard cat OS)"
    ),
    (21, WingInGroundHazardCatX, "Wing In Ground (hazard cat X)"),
    (22, WingInGroundHazardCatY, "Wing In Ground (hazard cat Y)"),
    (23, WingInGroundHazardCatZ, "Wing In Ground (hazard cat Z)"),
    (
        29,
        WingInGroundNoAdditionalInformation,
        "Wing In Ground (no additional information)"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetAlarmLookup,
    (57, LowBoatSpeed, "Low boat speed"),
    (58, WindDataMissing, "Wind data missing"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetAlertBitfieldLookup,
    (8, ApClutchDisengaged, "AP clutch disengaged"),
    (6, ApClutchOverload, "AP clutch overload"),
    (26, ApDepthDataMissing, "AP depth data missing"),
    (28, ApHeadingDataMissing, "AP heading data missing"),
    (30, ApNavDataMissing, "AP nav data missing"),
    (36, ApOffCourse, "AP off course"),
    (22, ApPositionDataMissing, "AP position data missing"),
    (32, ApRudderDataMissing, "AP rudder data missing"),
    (24, ApSpeedDataMissing, "AP speed data missing"),
    (34, ApWindDataMissing, "AP wind data missing"),
    (54, CanBusSupplyOverload, "CAN bus supply overload"),
    (44, DriveComputerMissing, "Drive computer missing"),
    (40, DriveInhibit, "Drive inhibit"),
    (46, DriveReadyMissing, "Drive ready missing"),
    (48, EvcComError, "EVC com error"),
    (50, EvcOverride, "EVC override"),
    (16, HighDriveSupply, "High drive supply"),
    (38, HighDriveTemperature, "High drive temperature"),
    (52, LowCanBusVoltage, "Low CAN bus voltage"),
    (18, LowDriveSupply, "Low drive supply"),
    (20, MemoryFail, "Memory fail"),
    (0, NoGpsFix, "No GPS fix"),
    (
        2,
        NoActiveAutopilotControlUnit,
        "No active autopilot control unit"
    ),
    (4, NoAutopilotComputer, "No autopilot computer"),
    (12, NoRudderResponse, "No rudder response"),
    (10, RudderControllerFault, "Rudder controller fault"),
    (14, RudderDriveOverload, "Rudder drive overload"),
    (42, RudderLimit, "Rudder limit"),
    (56, WindSensorBatteryLow, "Wind sensor battery low"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetApEventsLookup,
    (9, AutoMode, "Auto mode"),
    (19, CTurn, "C-Turn"),
    (26, ChangeCourse, "Change course"),
    (24, DepthTurn, "Depth (Turn)"),
    (14, FollowUpMode, "Follow Up mode"),
    (23, LazySTurn, "Lazy-S (Turn)"),
    (10, NavMode, "Nav mode"),
    (13, NonFollowUpMode, "Non Follow Up mode"),
    (112, PingPortEnd, "Ping port end"),
    (113, PingStarboardEnd, "Ping starboard end"),
    (21, SpiralTurn, "Spiral (Turn)"),
    (18, SquareTurn, "Square (Turn)"),
    (6, Standby, "Standby"),
    (61, TimerSync, "Timer sync"),
    (20, UTurn, "U-Turn"),
    (15, WindMode, "Wind mode"),
    (22, ZigZagTurn, "Zig Zag (Turn)"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetApModeLookup,
    (2, Heading, "Heading"),
    (10, Nav, "Nav"),
    (11, NoDrift, "No Drift"),
    (3, Wind, "Wind"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetApModeBitfieldLookup,
    (4, Heading, "Heading"),
    (6, Nav, "Nav"),
    (8, NoDrift, "No Drift"),
    (3, Standby, "Standby"),
    (10, Wind, "Wind"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetApStatusLookup,
    (16, Automatic, "Automatic"),
    (2, Manual, "Manual"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetBacklightLevelLookup,
    (0, UnformattableVariantA, "10% (Min)"),
    (99, UnformattableVariantB, "100% (Max)"),
    (11, UnformattableVariantC, "20%"),
    (22, UnformattableVariantD, "30%"),
    (33, UnformattableVariantE, "40%"),
    (44, UnformattableVariantF, "50%"),
    (55, UnformattableVariantG, "60%"),
    (66, UnformattableVariantH, "70%"),
    (77, UnformattableVariantI, "80%"),
    (88, UnformattableVariantJ, "90%"),
    (1, DayMode, "Day mode"),
    (4, NightMode, "Night mode"),
    UnknownLookupField
);
define_nmea_enum!(SimnetCommandLookup, (50, Text, "Text"), UnknownLookupField);
define_nmea_enum!(
    SimnetDeviceModelLookup,
    (0, Ac, "AC"),
    (100, Nac, "NAC"),
    (1, OtherDevice, "Other device"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetDeviceReportLookup,
    (10, Mode, "Mode"),
    (23, SailingProcessorStatus, "Sailing Processor Status"),
    (11, SendMode, "Send Mode"),
    (3, SendStatus, "Send Status"),
    (2, Status, "Status"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetDirectionLookup,
    (4, LeftRudderPort, "Left rudder (port)"),
    (2, Port, "Port"),
    (5, RightRudderStarboard, "Right rudder (starboard)"),
    (3, Starboard, "Starboard"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetDisplayGroupLookup,
    (1, Default, "Default"),
    (2, Group1, "Group 1"),
    (3, Group2, "Group 2"),
    (4, Group3, "Group 3"),
    (5, Group4, "Group 4"),
    (6, Group5, "Group 5"),
    (7, Group6, "Group 6"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetEventCommandLookup,
    (2, ApCommand, "AP command"),
    (1, Alarm, "Alarm"),
    (255, Autopilot, "Autopilot"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetHourDisplayLookup,
    (1, UnformattableVariantA, "12 hour"),
    (0, UnformattableVariantB, "24 hour"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetNightModeLookup,
    (2, Day, "Day"),
    (4, Night, "Night"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetNightModeColorLookup,
    (2, Blue, "Blue"),
    (1, Green, "Green"),
    (0, Red, "Red"),
    (3, White, "White"),
    UnknownLookupField
);
define_nmea_enum!(
    SimnetTimeFormatLookup,
    (1, MMddyyyy, "MM/dd/yyyy"),
    (2, DdMMyyyy, "dd/MM/yyyy"),
    UnknownLookupField
);
define_nmea_enum!(
    SonichubCommandLookup,
    (4, AmRadio, "AM Radio"),
    (16, Album, "Album"),
    (15, Artist, "Artist"),
    (9, Control, "Control"),
    (12, FmRadio, "FM Radio"),
    (25, Init1, "Init #1"),
    (1, Init2, "Init #2"),
    (50, Init3, "Init #3"),
    (23, MaxVolume, "Max Volume"),
    (19, MenuItem, "Menu Item"),
    (13, Playlist, "Playlist"),
    (48, Position, "Position"),
    (6, Source, "Source"),
    (8, SourceList, "Source List"),
    (14, Track, "Track"),
    (24, Volume, "Volume"),
    (5, ZoneInfo, "Zone Info"),
    (20, Zones, "Zones"),
    UnknownLookupField
);
define_nmea_enum!(
    SonichubControlLookup,
    (128, Ack, "Ack"),
    (0, Set, "Set"),
    UnknownLookupField
);
define_nmea_enum!(
    SonichubPlaylistLookup,
    (4, NextSong, "Next song"),
    (6, PreviousSong, "Previous song"),
    (1, Report, "Report"),
    UnknownLookupField
);
define_nmea_enum!(
    SonichubSourceLookup,
    (0, Am, "AM"),
    (4, Aux, "AUX"),
    (5, Aux2, "AUX 2"),
    (1, Fm, "FM"),
    (6, Mic, "Mic"),
    (3, Usb, "USB"),
    (2, IPod, "iPod"),
    UnknownLookupField
);
define_nmea_enum!(
    SonichubTuningLookup,
    (3, SeekingDown, "Seeking down"),
    (1, SeekingUp, "Seeking up"),
    (2, Tuned, "Tuned"),
    UnknownLookupField
);
define_nmea_enum!(
    SpeedTypeLookup,
    (1, DualSpeed, "Dual speed"),
    (2, ProportionalSpeed, "Proportional speed"),
    (0, SingleSpeed, "Single speed"),
    UnknownLookupField
);
define_nmea_enum!(
    StationStatusLookup,
    (3, Blink, "Blink"),
    (2, CycleError, "Cycle Error"),
    (1, LowSnr, "Low SNR"),
    (0, StationInUse, "Station in use"),
    UnknownLookupField
);
define_nmea_enum!(
    StationTypeLookup,
    (
        2,
        AllTypesOfClassBMobileStation,
        "All types of Class B mobile station"
    ),
    (0, AllTypesOfMobileStation, "All types of mobile station"),
    (4, AtoNStation, "AtoN station"),
    (
        5,
        ClassBCsShipborneMobileStation,
        "Class B CS shipborne mobile station"
    ),
    (6, InlandWaterways, "Inland waterways"),
    (7, RegionalUse7, "Regional use 7"),
    (8, RegionalUse8, "Regional use 8"),
    (9, RegionalUse9, "Regional use 9"),
    (3, SarAirborneMobileStation, "SAR airborne mobile station"),
    UnknownLookupField
);
define_nmea_enum!(
    SteeringModeLookup,
    (2, FollowUpDevice, "Follow-Up Device"),
    (4, HeadingControl, "Heading Control"),
    (3, HeadingControlStandalone, "Heading Control Standalone"),
    (0, MainSteering, "Main Steering"),
    (1, NonFollowUpDevice, "Non-Follow-Up Device"),
    (5, TrackControl, "Track Control"),
    UnknownLookupField
);
define_nmea_enum!(
    SystemTimeLookup,
    (1, Glonass, "GLONASS"),
    (0, Gps, "GPS"),
    (3, LocalCesiumClock, "Local Cesium clock"),
    (5, LocalCrystalClock, "Local Crystal clock"),
    (4, LocalRubidiumClock, "Local Rubidium clock"),
    (2, RadioStation, "Radio Station"),
    UnknownLookupField
);
define_nmea_enum!(
    TankTypeLookup,
    (5, BlackWater, "Black water"),
    (0, Fuel, "Fuel"),
    (2, GrayWater, "Gray water"),
    (3, LiveWell, "Live well"),
    (4, Oil, "Oil"),
    (1, Water, "Water"),
    UnknownLookupField
);
define_nmea_enum!(
    TargetAcquisitionLookup,
    (1, Automatic, "Automatic"),
    (0, Manual, "Manual"),
    UnknownLookupField
);
define_nmea_enum!(
    TemperatureSourceLookup,
    (
        10,
        ApparentWindChillTemperature,
        "Apparent Wind Chill Temperature"
    ),
    (6, BaitWellTemperature, "Bait Well Temperature"),
    (9, DewPointTemperature, "Dew Point Temperature"),
    (3, EngineRoomTemperature, "Engine Room Temperature"),
    (14, ExhaustGasTemperature, "Exhaust Gas Temperature"),
    (13, FreezerTemperature, "Freezer Temperature"),
    (12, HeatIndexTemperature, "Heat Index Temperature"),
    (8, HeatingSystemTemperature, "Heating System Temperature"),
    (2, InsideTemperature, "Inside Temperature"),
    (5, LiveWellTemperature, "Live Well Temperature"),
    (4, MainCabinTemperature, "Main Cabin Temperature"),
    (1, OutsideTemperature, "Outside Temperature"),
    (7, RefrigerationTemperature, "Refrigeration Temperature"),
    (0, SeaTemperature, "Sea Temperature"),
    (15, ShaftSealTemperature, "Shaft Seal Temperature"),
    (
        11,
        TheoreticalWindChillTemperature,
        "Theoretical Wind Chill Temperature"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    ThrusterControlEventsLookup,
    (
        0,
        AnotherDeviceControllingThruster,
        "Another device controlling thruster"
    ),
    (
        1,
        BoatSpeedTooFastToSafelyUseThruster,
        "Boat speed too fast to safely use thruster"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    ThrusterDirectionControlLookup,
    (0, Off, "Off"),
    (1, Ready, "Ready"),
    (2, ToPort, "To Port"),
    (3, ToStarboard, "To Starboard"),
    UnknownLookupField
);
define_nmea_enum!(
    ThrusterMotorEventsLookup,
    (
        4,
        ControllerUnderVoltageCutout,
        "Controller under voltage cutout"
    ),
    (2, LowOilLevelWarning, "Low oil level warning"),
    (5, ManufacturerDefined, "Manufacturer defined"),
    (1, MotorOverCurrentCutout, "Motor over current cutout"),
    (
        0,
        MotorOverTemperatureCutout,
        "Motor over temperature cutout"
    ),
    (3, OilOverTemperatureWarning, "Oil over temperature warning"),
    UnknownLookupField
);
define_nmea_enum!(
    ThrusterMotorTypeLookup,
    (0, UnformattableVariantA, "12VDC"),
    (3, UnformattableVariantB, "24VAC"),
    (1, UnformattableVariantC, "24VDC"),
    (2, UnformattableVariantD, "48VDC"),
    (4, Hydraulic, "Hydraulic"),
    UnknownLookupField
);
define_nmea_enum!(
    ThrusterRetractControlLookup,
    (1, Extend, "Extend"),
    (0, Off, "Off"),
    (2, Retract, "Retract"),
    UnknownLookupField
);
define_nmea_enum!(
    TideLookup,
    (0, Falling, "Falling"),
    (1, Rising, "Rising"),
    UnknownLookupField
);
define_nmea_enum!(
    TimeStampLookup,
    (62, DeadReckoningMode, "Dead reckoning mode"),
    (61, ManualInputMode, "Manual input mode"),
    (60, NotAvailable, "Not available"),
    (
        63,
        PositioningSystemIsInoperative,
        "Positioning system is inoperative"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    TrackingLookup,
    (1, Acquiring, "Acquiring"),
    (0, Cancelled, "Cancelled"),
    (3, Lost, "Lost"),
    (2, Tracking, "Tracking"),
    UnknownLookupField
);
define_nmea_enum!(
    TransmissionIntervalLookup,
    (3, AccessDenied, "Access denied"),
    (0, Acknowledge, "Acknowledge"),
    (4, NotSupported, "Not supported"),
    (2, TransmitIntervalTooLow, "Transmit Interval too low"),
    (
        1,
        TransmitIntervalPriorityNotSupported,
        "Transmit Interval/Priority not supported"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    TurnModeLookup,
    (2, RadiusControlled, "Radius controlled"),
    (0, RudderLimitControlled, "Rudder limit controlled"),
    (1, TurnRateControlled, "Turn rate controlled"),
    UnknownLookupField
);
define_nmea_enum!(
    TxRxModeLookup,
    (1, TxARxARxB, "Tx A, Rx A/Rx B"),
    (0, TxATxBRxARxB, "Tx A/Tx B, Rx A/Rx B"),
    (2, TxBRxARxB, "Tx B, Rx A/Rx B"),
    UnknownLookupField
);
define_nmea_enum!(
    VideoProtocolsLookup,
    (1, Ntsc, "NTSC"),
    (0, Pal, "PAL"),
    UnknownLookupField
);
define_nmea_enum!(
    WatermakerStateLookup,
    (4, Flushing, "Flushing"),
    (6, Initiating, "Initiating"),
    (7, Manual, "Manual"),
    (5, Rinsing, "Rinsing"),
    (2, Running, "Running"),
    (1, Starting, "Starting"),
    (0, Stopped, "Stopped"),
    (3, Stopping, "Stopping"),
    UnknownLookupField
);
define_nmea_enum!(
    WaterReferenceLookup,
    (3, CorrelationUltraSound, "Correlation (ultra sound)"),
    (2, Doppler, "Doppler"),
    (4, ElectroMagnetic, "Electro Magnetic"),
    (0, PaddleWheel, "Paddle wheel"),
    (1, PitotTube, "Pitot tube"),
    UnknownLookupField
);
define_nmea_enum!(
    WaveformLookup,
    (1, ModifiedSineWave, "Modified sine wave"),
    (0, SineWave, "Sine wave"),
    UnknownLookupField
);
define_nmea_enum!(
    WindlassControlLookup,
    (
        0,
        AnotherDeviceControllingWindlass,
        "Another device controlling windlass"
    ),
    UnknownLookupField
);
define_nmea_enum!(
    WindlassDirectionLookup,
    (1, Down, "Down"),
    (0, Off, "Off"),
    (2, Up, "Up"),
    UnknownLookupField
);
define_nmea_enum!(
    WindlassMonitoringLookup,
    (
        1,
        ControllerOverCurrentCutout,
        "Controller over current cut-out"
    ),
    (
        2,
        ControllerOverTemperatureCutout,
        "Controller over temperature cut-out"
    ),
    (
        0,
        ControllerUnderVoltageCutout,
        "Controller under voltage cut-out"
    ),
    (3, ManufacturerDefined, "Manufacturer defined"),
    UnknownLookupField
);
define_nmea_enum!(
    WindlassMotionLookup,
    (1, DeploymentOccurring, "Deployment occurring"),
    (2, RetrievalOccurring, "Retrieval occurring"),
    (0, WindlassStopped, "Windlass stopped"),
    UnknownLookupField
);
define_nmea_enum!(
    WindlassOperationLookup,
    (4, EndOfRodeReached, "End of rode reached"),
    (2, NoWindlassMotionDetected, "No windlass motion detected"),
    (
        3,
        RetrievalDockingDistanceReached,
        "Retrieval docking distance reached"
    ),
    (1, SensorError, "Sensor error"),
    (0, SystemError, "System error"),
    UnknownLookupField
);
define_nmea_enum!(
    WindReferenceLookup,
    (2, Apparent, "Apparent"),
    (
        1,
        MagneticGroundReferencedToMagneticNorth,
        "Magnetic (ground referenced to Magnetic North)"
    ),
    (3, TrueBoatReferenced, "True (boat referenced)"),
    (
        0,
        TrueGroundReferencedToNorth,
        "True (ground referenced to North)"
    ),
    (4, TrueWaterReferenced, "True (water referenced)"),
    UnknownLookupField
);
define_nmea_enum!(
    YesNoLookup,
    (0, No, "No"),
    (1, Yes, "Yes"),
    UnknownLookupField
);
