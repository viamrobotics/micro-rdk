pub trait Lookup: Sized {
    fn from_value(val: u32) -> Self;
    fn to_string(&self) -> String;
}

/// For generating a lookup data type found in an NMEA message. The first argument is the name of the
/// enum type that will be generated. Each successive argument is a tuple with
/// (raw number value, name of enum instance, string representation)
macro_rules! lookup {
    ( $name:ident, $(($value:expr, $var:ident, $label:expr)),*, $default:ident) => {
        #[derive(Copy, Clone, Debug)]
        pub enum $name {
            $($var),*,
            $default
        }

        impl Lookup for $name {
            fn from_value(val: u32) -> Self {
                match val {
                    $($value => Self::$var),*,
                    _ => Self::$default
                }
            }

            fn to_string(&self) -> String {
                match self {
                    $(Self::$var => $label),*,
                    Self::$default => "could not parse"
                }.to_string()
            }
        }
    };

}

// Examples below taken from https://canboat.github.io/canboat/canboat.html

lookup!(
    WaterReference,
    (0, PaddleWheel, "Paddle Wheel"),
    (1, PitotTube, "Pitot Tube"),
    (2, Doppler, "Doppler"),
    (3, Correlation, "Correlation (ultra sound)"),
    (4, Electromagnetic, "Electro Magnetic"),
    CouldNotParse
);

lookup!(
    TemperatureSource,
    (0, SeaTemperature, "Sea Temperature"),
    (1, OutsideTemperature, "Outside Temperature"),
    (2, InsideTemperature, "Inside Temperature"),
    (3, EngineRoomTemperature, "Engine Room Temperature"),
    (4, MainCabinTemperature, "Main Cabin Temeprature"),
    (5, LiveWellTemperature, "Live Well Temperature"),
    (6, BaitWellTemperature, "Bait Well Temperature"),
    (7, RefrigerationTemperature, "Refrigeration Temperature"),
    (8, HeatingSystemTemperature, "Heating System Temperature"),
    (9, DewPointTemperature, "Dew Point Temperature"),
    (
        10,
        ApparentWindChillTemerature,
        "Apparent Wind Chill Temperature"
    ),
    (
        11,
        TheoreticalWindChillTemperature,
        "Theoretical Wind Chill Temperature"
    ),
    (12, HeatIndexTemperature, "Heat Index Temperature"),
    (13, FreezerTemperature, "Freezer Temperature"),
    (14, ExhaustGasTemperature, "Exhaust Gas Temperature"),
    (15, ShaftSealTemeprature, "Shaft Seal Temperature"),
    CouldNotParse
);

lookup!(
    SystemTimeSource,
    (0, Gps, "GPS"),
    (1, Glonass, "GLONASS"),
    (2, RadioStation, "Radio Station"),
    (3, LocalCesiumClock, "Local Cesium clock"),
    (4, LocalRubidiumClock, "Local Rubidium clock"),
    (5, LocalCrystalClock, "Local Crystal clock"),
    CouldNotParse
);

lookup!(
    MagneticVariationSource,
    (0, Manual, "Manual"),
    (1, AutomaticChart, "Automatic Chart"),
    (2, AutomaticTable, "Automatic Table"),
    (3, AutomaticCalculation, "Automatic Calculation"),
    (4, Wmm2000, "WMM 2000"),
    (5, Wmm2005, "WMM 2005"),
    (6, Wmm2010, "WMM 2010"),
    (7, Wmm2015, "WMM 2015"),
    (8, Wmm2020, "WMM 2020"),
    CouldNotParse
);

lookup!(
    RepeatIndicator,
    (0, Initial, "Initial"),
    (1, FirstRetransmission, "First retransmission"),
    (2, SecondRetransmission, "Second retransmission"),
    (3, ThirdRetransmission, "Third retransmission"),
    (4, FinalRetransmission, "Final retransmission"),
    CouldNotParse
);

lookup!(
    AisMessageId,
    (
        1,
        ScheduledClassAPositionReport,
        "Scheduled Class A position report"
    ),
    (
        2,
        AssignedScheduledClassAPositionReport,
        "Assigned scheduled Class A position report"
    ),
    (
        3,
        InterrogatedClassAPositionReport,
        "Interrogated Class A position report"
    ),
    (4, BaseStationReport, "Base station report"),
    (5, StaticVoyageRelatedData, "Static and voyage related data"),
    (6, BinaryAddressedMessage, "Binary addressed message"),
    (7, BinaryAcknowledgement, "Binary acknowledgement"),
    (8, BinaryBroadcastMessage, "Binary broadcast message"),
    (
        9,
        StandardSarAircraftPositionReport,
        "Standard SAR aircraft position report"
    ),
    (10, UtcDateInquiry, "UTC/date inquiry"),
    (11, UtcDateResponse, "UTC/date response"),
    (
        12,
        SafetyRelatedAddressedMessage,
        "Safety related addressed message"
    ),
    (
        13,
        SafetyRelatedAcknowlegdement,
        "Safety related acknowledgement"
    ),
    (
        14,
        SafetyRelatedBroadcastMessage,
        "Satety related broadcast message"
    ),
    (15, Interrogation, "Interrogation"),
    (16, AssignmentModeCommand, "Assignment mode command"),
    (
        17,
        DgnssBroadcastBinaryMessage,
        "DGNSS broadcast binary message"
    ),
    (
        18,
        StandardClassBPositionReport,
        "Standard Class B position report"
    ),
    (
        19,
        ExtendedClassBPositionReport,
        "Extended Class B position report"
    ),
    (
        20,
        DataLinkManagementMessage,
        "Data link management message"
    ),
    (21, AtonReport, "ATON report"),
    (22, ChannelManagement, "Channel management"),
    (23, GroupAssignmentCommand, "Group assignment command"),
    (24, StaticDataReport, "Static data report"),
    (25, SingleSlotBinaryMessage, "Single slot binary message"),
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
    CouldNotParse
);

lookup!(
    PositionAccuracy,
    (0, Low, "Low"),
    (1, High, "High"),
    CouldNotParse
);

lookup!(
    RaimFlag,
    (0, NotInUse, "not in use"),
    (1, InUse, "in use"),
    CouldNotParse
);

lookup!(
    TimeStamp,
    (60, NotAvailable, "Not available"),
    (61, ManualInputMode, "Manual input mode"),
    (62, DeadReckoningMode, "Dead reckoning mode"),
    (
        63,
        PositioningSystemIsInoperative,
        "Positioning system is inoperative"
    ),
    CouldNotParse
);

lookup!(
    AisTransceiver,
    (0, ChannelAVdlReception, "Channel A VDL reception"),
    (1, ChannelBVdlReception, "Channel B VDL reception"),
    (2, ChannelAVdlTransmission, "Channel A VDL transmission"),
    (3, ChannelBVdlTransmission, "Channel B VDL transmission"),
    (
        4,
        OwnInformationNotBroadcast,
        "Own information not broadcast"
    ),
    (5, Reserved, "Reserved"),
    CouldNotParse
);

lookup!(
    NavStatus,
    (0, UnderWayUsingEngine, "Under way using engine"),
    (1, AtAnchor, "At anchor"),
    (2, NotUnderCommand, "Not under command"),
    (3, RestrictedManeuverability, "Restricted maneuverability"),
    (4, ConstrainedByHerDraught, "Constrained by her draught"),
    (5, Moored, "Moored"),
    (6, Aground, "Aground"),
    (7, EngagedInFishing, "Engaged in Fishing"),
    (8, UnderWaySailing, "Under way sailing"),
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
    (
        11,
        PowerDrivenVesslTowingAstern,
        "Power-driven vessl towing astern"
    ),
    (
        12,
        PowerDrivenVesslPushingAhead,
        "Power-driven vessl pushing ahead or towing alongside"
    ),
    (14, AisSart, "AIS-SART"),
    CouldNotParse
);

lookup!(
    AisSpecialManeuver,
    (0, NotAvailable, "Not available"),
    (
        1,
        NotEngagedInSpecialManeuver,
        "Not engaged in special maneuver"
    ),
    (2, EngagedInSpecialManeuver, "Engaged in special maneuver"),
    (3, Reserved, "Reserved"),
    CouldNotParse
);

lookup!(
    DirectionReference,
    (0, True, "True"),
    (1, Magnetic, "Magnetic"),
    (2, Error, "Error"),
    CouldNotParse
);

lookup!(
    Gns,
    (0, Gps, "GPS"),
    (1, Glonass, "GLONASS"),
    (2, GpsGlonass, "GPS+GLONASS"),
    (3, GpsSbasWaas, "GPS+SBAS/WAAS"),
    (4, GpsSbasWaasGlonass, "GPS+SBAS/WAAS+GLONASS"),
    (5, Chayka, "Chayka"),
    (6, Integrated, "integrated"),
    (7, Surveyed, "surveyed"),
    (8, Galileo, "Galileo"),
    CouldNotParse
);

lookup!(
    GnsMethod,
    (0, NoGnss, "no GNSS"),
    (1, GnssFix, "GNSS fix"),
    (2, DgnssFix, "DGNSS fix"),
    (3, PreciseGnss, "Precise GNSS"),
    (4, RtkFixedInteger, "RTK Fixed Integer"),
    (5, RtkFloat, "RTK float"),
    (6, EstimatedDrMode, "Estimated (DR) mode"),
    (7, ManualInput, "Manual Input"),
    (8, SimulateMode, "Simulate mode"),
    CouldNotParse
);

lookup!(
    GnsIntegrity,
    (0, NoIntegrityChecking, "No integrity checking"),
    (1, Safe, "Safe"),
    (2, Caution, "Caution"),
    CouldNotParse
);
