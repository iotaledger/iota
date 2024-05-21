import { Controller, Get } from '@nestjs/common';

@Controller()
export class AnalyticsController {
  @Get('product-analytics')
  getProductAnalytics() {
    return;
  }
}
