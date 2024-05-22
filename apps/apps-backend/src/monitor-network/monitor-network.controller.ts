import { Controller, Get, Query } from '@nestjs/common';

type Project = 'WALLET' | 'EXPLORER';

@Controller('/monitor-network')
export class MonitorNetworkController {
  @Get('/')
  async getMonitorNetwork(@Query('project') project: Project) {
    if (project === 'WALLET') {
      return {
        degraded: false,
      };
    }
    return {
      degraded: false,
    };
  }
}
