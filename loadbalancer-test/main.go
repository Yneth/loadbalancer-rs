package main

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"math/rand"
	"net"
	"os"
	"strings"
	"sync"
	"unsafe"
)

type App struct {
	Name    string
	Ports   []uint16
	Targets []string
}

type Config struct {
	Apps []App
}

func readConfig(path string) (*Config, error) {
	file, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var config *Config
	err = json.Unmarshal(file, &config)
	return config, err
}

var alphabet = []byte("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ")

func generateAscii(size int) string {
	result := make([]byte, size)
	rand.Read(result)
	for i := 0; i < size; i++ {
		result[i] = alphabet[result[i]%byte(len(alphabet))]
	}
	return *(*string)(unsafe.Pointer(&result))
}

func generateBytes(size int) []byte {
	result := make([]byte, size)
	rand.Read(result)
	return result
}

func writeAndRead(stream net.Conn, writeData []byte) ([]byte, error) {
	writer := bufio.NewWriter(stream)
	_, err := writer.Write(writeData)
	if err != nil {
		return nil, err
	}

	_, err = writer.Write([]byte("\n"))
	if err != nil {
		return nil, err
	}

	err = writer.Flush()
	if err != nil {
		return nil, err
	}

	reader := bufio.NewReader(stream)
	recv, err := reader.ReadBytes('\n')
	if err != nil {
		return nil, err
	}
	return recv, err
}

func checkReqEqualsResp(writeData []byte, stream net.Conn) error {
	readData, err := writeAndRead(stream, writeData)
	if err != nil {
		return err
	}

	if !strings.EqualFold(string(writeData), string(readData)) {
		msg := fmt.Sprintf("expected=%s != actual=%s", string(writeData), string(readData))
		return errors.New(msg)
	}
	return nil
}

func probe(action string, writeData []byte, stream net.Conn) bool {
	err := checkReqEqualsResp(writeData, stream)
	if err != nil {
		log.Printf("> OK: %s", action)
		return true
	} else {
		log.Printf("> FAIL: %s", action)
		return false
	}
}

func probeSimultaneousConnections(port uint16) int {
	result := 0
	for j := 0; j <= 10; j++ {
		isSuccess := func() bool {
			conn, err := net.Dial("tcp", fmt.Sprintf("localhost:%d", port))
			if err != nil {
				log.Printf("FAIL: simultaneous_conn, reason: %s", err)
				return false
			}
			defer func() { _ = conn.Close() }()
			if !probe("simultaneous_conn", []byte(generateAscii(100)), conn) {
				return false
			}
			return true
		}()
		if isSuccess {
			result += 1
		}
	}
	return result
}

func probeParallelConnections(port uint16) int {
	var wg sync.WaitGroup
	ch := make(chan bool, 10)
	for j := 0; j < 10; j++ {
		wg.Add(1)
		go func(wg *sync.WaitGroup) {
			conn, err := net.Dial("tcp", fmt.Sprintf("localhost:%d", port))
			if err != nil {
				log.Printf("FAIL: parallel_conn, reason: %s", err)
				wg.Done()
				ch <- false
				return
			}
			defer func() { _ = conn.Close() }()
			if !probe("parallel_conn", []byte(generateAscii(100)), conn) {
				wg.Done()
				ch <- false
				return
			}
			wg.Done()
			ch <- true
		}(&wg)
	}
	wg.Wait()
	close(ch)

	successCount := 0
	for success := range ch {
		if success {
			successCount += 1
		}
	}
	return successCount
}

func validateAppPorts(app App) {
	log.Printf("%s started processing", app.Name)

	if len(app.Ports) == 0 {
		log.Printf("%s no ports in application configuration", app.Name)
		return
	}

	for _, port := range app.Ports {

		func() {
			log.SetPrefix(fmt.Sprintf("%s port=%d :  ", app.Name, port))
			defer func() { log.SetPrefix("") }()
			stream, err := net.Dial("tcp", fmt.Sprintf("localhost:%d", port))
			if err != nil {
				log.Printf("FAIL: failed to establish connection on port, reason %s", err)
				return
			}
			defer func() { _ = stream.Close() }()

			if !probe("short_ascii", []byte(generateAscii(100)), stream) {
				return
			}
			if !probe("long_ascii", []byte(generateAscii(65536)), stream) {
				return
			}
			if !probe("random_bytes", generateBytes(200), stream) {
				return
			}
			if probeParallelConnections(port) < 5 {
				log.Print("FAIL: parallel_connections")
				return
			}
			if probeSimultaneousConnections(port) < 5 {
				log.Print("FAIL: simultaneous_connections")
				return
			}
			log.Printf("OK")
		}()
	}
}

func main() {
	log.SetOutput(os.Stdout)

	configPath := "./config.json"
	if len(os.Args) > 1 {
		configPath = os.Args[1]
	}

	conf, err := readConfig(configPath)
	if err != nil {
		log.Printf("failed to read configuration, reason %s", err)
		return
	}
	log.Printf("configuration %+v\n", conf)

	if len(conf.Apps) == 0 {
		log.Printf("no apps in configuration, bail out...")
		return
	}

	// decided not to use goroutine for readability
	for _, app := range conf.Apps {
		validateAppPorts(app)
	}
}
