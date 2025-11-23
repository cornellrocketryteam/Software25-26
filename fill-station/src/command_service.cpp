#include "command_service.hpp"
#include <spdlog/spdlog.h>

CommandServiceImpl::CommandServiceImpl(Hardware &hardware)
    : hardware(hardware) {}

grpc::Status CommandServiceImpl::Execute(grpc::ServerContext *context,
                                         const CommandRequest *request,
                                         CommandResponse *response) {
  if (!request) {
    spdlog::error("Received null request");
    return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, "Request cannot be null");
  }

  if (request->ignite()) {
    spdlog::info("Igniting ig1 and ig2");
    hardware.ig1.ignite();
    hardware.ig2.ignite();
    spdlog::info("Ignition sequence completed");
  }

  return grpc::Status::OK;
}
