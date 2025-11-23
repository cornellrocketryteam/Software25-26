#include <chrono>
#include <thread>

#include <gpiod.hpp>
#include <grpcpp/grpcpp.h>
#include <grpcpp/health_check_service_interface.h>
#include <spdlog/spdlog.h>

#include "hardware.hpp"
#include "command_service.hpp"

constexpr const char* server_address = "0.0.0.0:50051";

std::unique_ptr<grpc::Server> startGrpcCommandingServer(CommandServiceImpl &service) {
  grpc::EnableDefaultHealthCheckService(true);
  grpc::ServerBuilder builder;
  // Max number of simultaneous commands since requests run in their own thread
  builder.SetSyncServerOption(grpc::ServerBuilder::MAX_POLLERS, 15);
  // TODO: Make not Insecure
  builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
  builder.RegisterService(&service);

  std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
  return server;
}

int main() {
  spdlog::info("Starting fill-station service");

  Hardware hardware;
  CommandServiceImpl service(hardware);
  std::unique_ptr<grpc::Server> server = startGrpcCommandingServer(service);
  spdlog::info("Server listening on {}", server_address);

  while (true) {
    spdlog::debug("Igniter 1 continuity: {}", hardware.ig1.has_continuity());
    std::this_thread::sleep_for(std::chrono::seconds(1));
  }

  return 0;
}
