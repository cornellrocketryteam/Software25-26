#pragma once

#include "command.grpc.pb.h"
#include "hardware.hpp"
#include <grpc/grpc.h>

class CommandServiceImpl final : public Command::Service {
public:
  explicit CommandServiceImpl(Hardware &hardware);
  grpc::Status Execute(grpc::ServerContext *context,
                       const CommandRequest *request,
                       CommandResponse *response) override;

private:
  Hardware &hardware;
};
