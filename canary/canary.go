package main

import (
	"context"
	"errors"
	"os"
	"time"

	"runtime/debug"

	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"
	"go.viam.com/rdk/components/board"
	"go.viam.com/rdk/logging"
	"go.viam.com/rdk/robot/client"

	"go.viam.com/utils/rpc"
	"gonum.org/v1/gonum/stat"
)

type connectionStats struct {
	connectionSuccess   bool
	connectionAttempts  int
	connectionLatencyMs float64
	connectionError     string
}

type boardAPIStats struct {
	successes          int
	failures           int
	avgLatencyMs       float64
	avgLatencyMsStdDev float64
	connectionError    string
}

func main() {
	logger := logging.NewDebugLogger("canary")
	ctx := context.Background()
	runTimestamp := time.Now()
	mongodb_uri := os.Getenv("MONGODB_TEST_OUTPUT_URI")
	mongo_client, err := mongo.Connect(ctx, options.Client().ApplyURI(mongodb_uri))
	if err != nil {
		logger.Error(err)
		return
	}
	defer func() {
		if err := mongo_client.Disconnect(ctx); err != nil {
			panic(err)
		}
	}()
	coll := mongo_client.Database("micrordk_canary").Collection("raw_results")

	machine, connStats, err := tryConnect(ctx, logger)
	if err != nil {
		record, err := buildRecord(runTimestamp, connStats, boardAPIStats{})
		if err != nil {
			logger.Error(err)
			return
		}
		if _, err := coll.InsertOne(ctx, record); err != nil {
			logger.Error("could not upload canary result to database")
		}
		logger.Fatal(err)
	}
	defer machine.Close(ctx)

	board, err := board.FromRobot(machine, "board")
	if err != nil {
		logger.Error(err)
		return
	}

	pin, err := board.GPIOPinByName("32")
	if err != nil {
		logger.Error(err)
		return
	}

	boardStats := boardAPItest(ctx, pin)

	record, err := buildRecord(runTimestamp, connStats, boardStats)
	if err != nil {
		logger.Error(err)
		return
	}
	if _, err := coll.InsertOne(ctx, record); err != nil {
		logger.Error("could not upload canary result to database")
	}
}

func tryConnect(ctx context.Context, logger logging.Logger) (*client.RobotClient, connectionStats, error) {
	apiKey := os.Getenv("ESP32_CANARY_API_KEY")
	apiKeyId := os.Getenv("ESP32_CANARY_API_KEY_ID")
	robotAddress := os.Getenv("ESP32_CANARY_API_KEY_ID")
	stats := connectionStats{}
	var startTime time.Time
	var machine *client.RobotClient
	var err error
	for i := range 5 {
		startTime = time.Now()
		machine, err = client.New(
			ctx,
			robotAddress,
			logger,
			client.WithDialOptions(rpc.WithEntityCredentials(
				apiKeyId,
				rpc.Credentials{
					Type:    rpc.CredentialsTypeAPIKey,
					Payload: apiKey,
				})),
		)
		if err != nil {
			stats.connectionAttempts = i + 1
			if i == 4 {
				stats.connectionError = err.Error()
				return nil, stats, err
			}
		} else if i != 4 {
			machine.Close(ctx)
		}
		time.Sleep(500 * time.Millisecond)
	}
	stats.connectionLatencyMs = float64(time.Since(startTime).Milliseconds())
	stats.connectionSuccess = true
	return machine, stats, nil
}

func boardAPItest(ctx context.Context, pin board.GPIOPin) boardAPIStats {
	stats := boardAPIStats{}
	latencies := []float64{}
	for range 20 {
		time.Sleep(500 * time.Millisecond)
		_, err := pin.Get(ctx, nil)
		if err != nil {
			stats.failures += 1
			stats.connectionError = err.Error()
			continue
		}
		startTime := time.Now()
		err = pin.Set(ctx, true, nil)
		if err != nil {
			stats.failures += 1
			stats.connectionError = err.Error()
			continue
		}
		latencies = append(latencies, (float64(time.Since(startTime).Milliseconds())))
		value, err := pin.Get(ctx, nil)
		if err != nil {
			stats.failures += 1
			stats.connectionError = err.Error()
			continue
		}
		if !value {
			stats.failures += 1
			stats.connectionError = "Pin not set to high successfully"
			continue
		}
		stats.successes += 1
	}
	stats.avgLatencyMs, stats.avgLatencyMsStdDev = stat.MeanStdDev(latencies, nil)
	return stats
}

func getVersion() (string, error) {
	bi, ok := debug.ReadBuildInfo()
	if !ok {
		return "", errors.New("failed to read build info")
	}
	deps := bi.Deps
	sdk_version := ""
	for _, dep := range deps {
		if dep.Path == "go.viam.com/rdk" {
			sdk_version = dep.Version
		}
	}
	if sdk_version == "" {
		return "", errors.New("could not find Go SDK version")
	}
	return sdk_version, nil
}

func buildRecord(runTimestamp time.Time, connStats connectionStats, boardStats boardAPIStats) (bson.M, error) {
	sdkVersion, err := getVersion()
	if err != nil {
		return nil, err
	}

	connectionErr := ""
	if !connStats.connectionSuccess {
		connectionErr = connStats.connectionError
	} else if boardStats.failures > 0 {
		connectionErr = boardStats.connectionError
	}

	return bson.M{
		"timestamp":                    runTimestamp,
		"sdk_type":                     "Go",
		"sdk_version":                  sdkVersion,
		"connection_success":           connStats.connectionSuccess,
		"connection_error":             connectionErr,
		"connection_latency_ms":        connStats.connectionLatencyMs,
		"connection_attempts":          connStats.connectionAttempts,
		"board_api_successes":          boardStats.successes,
		"board_api_failures":           boardStats.failures,
		"board_api_avg_latency_ms":     boardStats.avgLatencyMs,
		"board_api_latency_ms_std_dev": boardStats.avgLatencyMsStdDev,
	}, nil
}
