# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

import argparse
from enum import Enum
import glob
import json
import math
import os
# Set the backend before importing pyplot to avoid macOS icon issues
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import matplotlib.ticker as tick
from glob import glob
from itertools import cycle

# A simple python script to plot measurements results. This script requires
# the following dependencies: `pip install matplotlib`.


def gen_data(measurement):
    for workloads in measurement['scrapers'].values():
        for data in workloads.values():
            yield data

def aggregate_tps(measurement, i=-1):
    # Calculate TPS for each scraper based on its own timestamp, then sum
    # This matches: sum of (count / timestamp) for each measurement
    tps = []
    for data in gen_data(measurement):
        last = data[i]
        count = float(last['count'])
        duration = float(last['timestamp']['secs'])
        tps += [(count / duration) if duration != 0 else 0]
    return sum(tps)


def aggregate_average_latency(measurement, i=-1):
    # Calculate weighted average: (sum of all latency_sum) / (sum of all counts)
    total_sum = 0
    total_count = 0
    for data in gen_data(measurement):
        last = data[i]
        total_sum += float(last['sum']['secs'])
        total_count += float(last['count'])
    return total_sum / total_count if total_count != 0 else 0


def aggregate_stdev_latency(measurement, i=-1):
    # Calculate pooled standard deviation using combined squared_sum, sum, and count
    # Formula: sqrt((Σsquared_sum / Σcount) - (Σsum / Σcount)^2)
    total_sum = 0
    total_squared_sum = 0
    total_count = 0
    
    for data in gen_data(measurement):
        last = data[i]
        total_sum += float(last['sum']['secs'])
        total_squared_sum += float(last['squared_sum']['secs'])
        total_count += float(last['count'])
    
    if total_count == 0:
        return 0
    
    first_term = total_squared_sum / total_count
    avg = total_sum / total_count
    variance = max(0, first_term - avg**2)
    return math.sqrt(variance)


def aggregate_p_latency(measurement, p=50, i=-1):
    # Aggregate all histogram buckets, then calculate quantile with linear interpolation
    # This matches Prometheus's histogram_quantile behavior
    combined_buckets = {}
    total_count = 0
    
    for data in gen_data(measurement):
        last = data[i]
        total_count += float(last['count'])
        for bucket_id, bucket_count in last['buckets'].items():
            combined_buckets[bucket_id] = combined_buckets.get(bucket_id, 0) + bucket_count
    
    if total_count == 0:
        return 0
    
    # Parse and sort buckets
    buckets = []
    for bucket_id, count in combined_buckets.items():
        if bucket_id == 'inf':
            bound = float('inf')
        else:
            bound = float(bucket_id)
        buckets.append((bound, count))
    buckets.sort(key=lambda x: x[0])
    
    # Calculate the rank for the desired quantile
    rank = (p / 100.0) * total_count
    
    # Find the bucket containing the quantile using linear interpolation
    prev_count = 0.0
    prev_bound = 0.0
    
    for bound, count in buckets:
        if count >= rank:
            # If this is the first bucket or all observations are in this bucket
            if prev_count == 0.0 or count == prev_count:
                return bound
            
            # Linear interpolation between prev_bound and bound
            fraction = (rank - prev_count) / (count - prev_count)
            interpolated = prev_bound + (bound - prev_bound) * fraction
            return interpolated
        
        prev_count = count
        prev_bound = bound
    
    # Fallback
    return prev_bound


class PlotType(Enum):
    L_GRAPH = 1
    HEALTH = 2
    SCALABILITY = 3
    INSPECT_TPS = 4
    INSPECT_LATENCY = 5
    DURATION_TPS = 6
    DURATION_LATENCY = 7


class PlotError(Exception):
    pass


@tick.FuncFormatter
def default_major_formatter(x, pos):
    if pos is None:
        return
    return f'{x/1000:.0f}k' if x >= 10_000 else f'{x:,.0f}'


@tick.FuncFormatter
def sec_major_formatter(x, pos):
    if pos is None:
        return
    return f'{x:,.0f}' if x >= 10 else f'{x:,.1f}'


class PlotParameters:
    def __init__(self, shared_objects_ratio, nodes, faults, specs=None, commit=None, use_internal_ip_address=None):
        self.nodes = nodes
        self.faults = faults
        self.shared_objects_ratio = shared_objects_ratio
        self.specs = specs
        self.commit = commit
        self.use_internal_ip_address = use_internal_ip_address


class MeasurementId:
    def __init__(self, measurement, max_latency=None):
        self.shared_objects_ratio = measurement['parameters']['benchmark_type']['shared_objects_ratio']
        self.nodes = measurement['parameters']['nodes']
        if 'Permanent' in measurement['parameters']['faults']:
            self.faults = measurement['parameters']['faults']['Permanent']['faults']
        else:
            self.faults = 0
        self.duration = measurement['parameters']['duration']
        self.machine_specs = measurement['machine_specs']
        self.commit = measurement['commit']
        self.max_latency = max_latency


class Plotter:
    def __init__(self, data_directory, parameters, y_max=None, legend_columns=1, median=True):
        self.data_directory = data_directory
        self.parameters = parameters
        self.y_max = y_max
        self.legend_columns = legend_columns
        self.median = median

    def _make_plot_directory(self):
        plot_directory = os.path.join(self.data_directory, 'plots')
        if not os.path.exists(plot_directory):
            os.makedirs(plot_directory)

        return plot_directory

    def _legend_entry(self, plot_type, id):
        if plot_type in [PlotType.L_GRAPH, PlotType.HEALTH]:
            f = '' if id.faults == 0 else f' ({id.faults} faulty)'
            l = f'{id.nodes} nodes{f}'
            return f'{l} - {id.shared_objects_ratio}% shared objects'
        elif plot_type == PlotType.SCALABILITY:
            f = '' if id.faults == 0 else f' ({id.faults} faulty)'
            l = f'{id.max_latency}s latency cap{f}'
            return f'{l} - {id.shared_objects_ratio}% shared objects'
        else:
            return None

    def _axes_labels(self, plot_type):
        if plot_type == PlotType.L_GRAPH:
            return ('Throughput (tx/s)', 'Latency (s)')
        elif plot_type == PlotType.HEALTH:
            return ('Input Load (tx/s)', 'Throughput (tx/s)')
        elif plot_type == PlotType.SCALABILITY:
            return ('Committee size', 'Throughput (tx/s)')
        elif plot_type in [PlotType.INSPECT_TPS, PlotType.DURATION_TPS]:
            return ('Duration (s)', 'Throughput (tx/s)')
        elif plot_type in [PlotType.INSPECT_LATENCY, PlotType.DURATION_LATENCY]:
            return ('Duration (s)', 'Latency (s)')
        else:
            assert False

    def _plot(self, data, plot_type, plot_id=None):
        plt.figure(figsize=(6.4, 2.4))
        markers = cycle(['o', 'v', 's', 'p', 'D', 'P'])

        for id_or_label, x_values, y_values, e_values in data:
            # For inspect plots, id_or_label is already the legend label string
            # For other plots, it's a MeasurementId object
            if plot_type in [PlotType.INSPECT_TPS, PlotType.INSPECT_LATENCY]:
                label = id_or_label
            else:
                label = self._legend_entry(plot_type, id_or_label)
            
            plt.errorbar(
                x_values, y_values, yerr=e_values,
                label=label,
                linestyle='dotted', marker=next(markers), capsize=3
            )

        if plot_type == PlotType.L_GRAPH:
            legend_anchor, legend_location = (-0.15, 1), 'upper right'
            plot_name = f'latency-{self.parameters.shared_objects_ratio}'
        elif plot_type == PlotType.HEALTH:
            legend_anchor, legend_location = (-0.15, 1), 'upper right'
            plot_name = f'health-{self.parameters.shared_objects_ratio}'
        elif plot_type == PlotType.SCALABILITY:
            legend_anchor, legend_location = (-0.15, 0), 'lower right'
            plot_name = f'scalability-{self.parameters.shared_objects_ratio}'
        elif plot_type == PlotType.INSPECT_TPS:
            legend_anchor, legend_location = (-0.15, 1), 'upper right'
            plot_name = f'inspect-tps-{plot_id}'
        elif plot_type == PlotType.INSPECT_LATENCY:
            legend_anchor, legend_location = (-0.15, 1), 'upper right'
            plot_name = f'inspect-latency-{plot_id}'
        elif plot_type == PlotType.DURATION_TPS:
            plot_name = f'inspect-aggregate-tps-{plot_id}'
        elif plot_type == PlotType.DURATION_LATENCY:
            plot_name = f'inspect-aggregate-latency-{plot_id}'
        else:
            assert False

        x_label, y_label = self._axes_labels(plot_type)

        skip_legend = plot_type in [
            PlotType.DURATION_TPS,
            PlotType.DURATION_LATENCY,
        ]
        if data and (not skip_legend):
            plt.legend(
                loc=legend_location,
                bbox_to_anchor=legend_anchor,
                ncol=self.legend_columns
            )
        plt.xlim(xmin=0)
        plt.ylim(bottom=0)
        if plot_type == PlotType.L_GRAPH:
            plt.ylim(top=self.y_max)
        plt.xlabel(x_label, fontweight='bold')
        plt.ylabel(y_label, fontweight='bold')
        plt.xticks(weight='bold')
        plt.yticks(weight='bold')
        plt.grid()
        ax = plt.gca()
        ax.xaxis.set_major_formatter(default_major_formatter)
        ax.yaxis.set_major_formatter(default_major_formatter)
        if plot_type in [PlotType.L_GRAPH, PlotType.INSPECT_LATENCY]:
            ax.yaxis.set_major_formatter(sec_major_formatter)

        for x in ['pdf', 'png']:
            filename = os.path.join(
                self._make_plot_directory(), f'{plot_name}.{x}'
            )
            plt.savefig(filename, bbox_inches='tight')

    def _load_measurement_data(self, filename):
        measurements = []
        files = glob(os.path.join(self.data_directory, '**', filename), recursive=True)
        for file in files:
            print(f'Loading file {file}...')
            with open(file, 'r') as f:
                try:
                    measurements += [json.loads(f.read())]
                except json.JSONDecodeError as e:
                    raise PlotError(f'Failed to load file {file}: {e}')

        return measurements

    def _file_format(self, shared_objects_ratio, faults, nodes, load):
        use_ip = self.parameters.use_internal_ip_address
        use_ip = '*' if use_ip is None else 'true' if use_ip else 'false'
        return f'measurements-{shared_objects_ratio}-{faults}-{nodes}-{load}-{use_ip}-*.json'

    def plot_latency_throughput(self):
        plot_lines_data = []
        shared_objects_ratio = self.parameters.shared_objects_ratio
        for n in self.parameters.nodes:
            for f in self.parameters.faults:
                filename = self._file_format(shared_objects_ratio, f, n, '*')
                plot_lines_data += [self._load_measurement_data(filename)]

        plot_data = []
        for measurements in plot_lines_data:
            x_values, y_values, e_values = [], [], []
            measurements.sort(key=lambda x: x['parameters']['load'])
            for measurement in measurements:
                x_values += [aggregate_tps(measurement)]
                if self.median:
                    y_values += [aggregate_p_latency(measurement, p=50)]
                    e_values += [aggregate_p_latency(measurement, p=75)]
                else:
                    y_values += [aggregate_average_latency(measurement)]
                    e_values += [aggregate_stdev_latency(measurement)]

            if x_values:
                id = MeasurementId(measurements[0])
                plot_data += [(id, x_values, y_values, e_values)]

        self._plot(plot_data, PlotType.L_GRAPH)

    def plot_health(self):
        plot_lines_data = []
        shared_objects_ratio = self.parameters.shared_objects_ratio
        for n in self.parameters.nodes:
            for f in self.parameters.faults:
                filename = self._file_format(shared_objects_ratio, f, n, '*')
                plot_lines_data += [self._load_measurement_data(filename)]

        plot_data = []
        for measurements in plot_lines_data:
            x_values, y_values, e_values = [], [], []
            measurements.sort(key=lambda x: x['parameters']['load'])
            for measurement in measurements:
                x_values += [measurement['parameters']['load']]
                y_values += [aggregate_tps(measurement)]
                e_values += [0]

            if x_values:
                id = MeasurementId(measurements[0])
                plot_data += [(id, x_values, y_values, e_values)]

        self._plot(plot_data, PlotType.HEALTH)

    def plot_scalability(self, max_latencies):
        plot_lines_data = []
        shared_objects_ratio = self.parameters.shared_objects_ratio
        for f in self.parameters.faults:
            for l in max_latencies:
                filenames = []
                for n in self.parameters.nodes:
                    filename = self._file_format(
                        shared_objects_ratio, f, n, '*'
                    )
                    measurements = self._load_measurement_data(filename)
                    measurements = [
                        x for x in measurements if aggregate_average_latency(x) <= l
                    ]
                    if measurements:
                        filenames += [
                            max(measurements, key=lambda x: aggregate_tps(x))
                        ]
                plot_lines_data += [(filenames, l)]

        plot_data = []
        for measurements, max_latency in plot_lines_data:
            x_values, y_values, e_values = [], [], []
            for measurement in measurements:
                x_values += [measurement['parameters']['nodes']]
                y_values += [aggregate_tps(measurement)]
                e_values += [0]

            if x_values:
                id = MeasurementId(measurements[0], max_latency)
                plot_data += [(id, x_values, y_values, e_values)]

        self._plot(plot_data, PlotType.SCALABILITY)

    def plot_inspect(self, file):
        with open(file, 'r') as f:
            try:
                measurement = json.loads(f.read())
            except json.JSONDecodeError as e:
                raise PlotError(f'Failed to load file {file}: {e}')

        plot_tps_data, plot_lat_data = [], []
        for scraper_id, workloads in measurement['scrapers'].items():
            for workload_name, data in workloads.items():
                x_values, y_tps_values, y_lat_values, e_values = [], [], [], []
                for d in data:
                    count = float(d['count'])
                    duration = float(d['timestamp']['secs'])
                    total = float(d['sum']['secs'])

                    tps = (count / duration) if duration != 0 else 0
                    avg_latency = total / count if count != 0 else 0

                    x_values += [duration]
                    y_tps_values += [tps]
                    y_lat_values += [avg_latency]
                    e_values += [0]

                if x_values:
                    # Create a legend label with scraper ID and workload type
                    label = f'client {scraper_id} - {workload_name}'
                    plot_tps_data += [(label, x_values, y_tps_values, e_values)]
                    plot_lat_data += [(label, x_values, y_lat_values, e_values)]

        basename = os.path.basename(file)
        id = '-'.join(basename.split('-')[1:]).split('.')[0]
        self._plot(plot_tps_data, PlotType.INSPECT_TPS, plot_id=id)
        self._plot(plot_lat_data, PlotType.INSPECT_LATENCY, plot_id=id)

    def plot_duration(self, file, precision):
        with open(file, 'r') as f:
            try:
                measurement = json.loads(f.read())
            except json.JSONDecodeError as e:
                raise PlotError(f'Failed to load file {file}: {e}')

        total_duration = float(measurement['parameters']['duration']['secs'])
        # Add 1 to include the last bucket
        # The script would panic if the duration and total_duration are really close
        # e.g., total_duration=200.0, duration=194.0 and precision=30.0, then
        # int(194.0/30.0) = int(200.0/30.0) = 6, which is out of bounds if we don't add 1
        length = int(total_duration / precision) + 1

        scrapers_tps_data, scrapers_lat_data = [], []
        for data in gen_data(measurement):
            all_y_tps_values = [[] for _ in range(length)]
            all_y_lat_values = [[] for _ in range(length)]

            for d in data:
                count = float(d['count'])
                duration = float(d['timestamp']['secs'])
                total = float(d['sum']['secs'])

                tps = (count / duration) if duration != 0 else 0
                avg_latency = total / count if count != 0 else 0

                if duration <= total_duration:
                    i = int(duration / precision)
                    # Ensure index is within bounds
                    if i < length:
                        all_y_tps_values[i] += [tps]
                        all_y_lat_values[i] += [avg_latency]

            aggregate_y_tps_values, aggregate_y_lat_values = [], []
            for x in all_y_tps_values:
                aggregate_y_tps_values += [
                    sum(x) / len(x) if len(x) != 0 else 0
                ]
            for x in all_y_lat_values:
                aggregate_y_lat_values += [
                    sum(x) / len(x) if len(x) != 0 else 0
                ]

            scrapers_tps_data += [aggregate_y_tps_values]
            scrapers_lat_data += [aggregate_y_lat_values]

        x_values, e_values = [], []
        y_tps_values, y_lat_values = [0]*length, [0]*length
        for i in range(length):
            x_values += [int((i*precision + (i+1)*precision)/2)]
            y_tps_values[i] = sum(x[i] for x in scrapers_tps_data)
            y_lat_values[i] = max(x[i] for x in scrapers_lat_data)
            e_values += [0]

        basename = os.path.basename(file)
        id = '-'.join(basename.split('-')[1:]).split('.')[0]

        plot_tps_data = [(id, x_values, y_tps_values, e_values)]
        plot_lat_data = [(id, x_values, y_lat_values, e_values)]
        self._plot(plot_tps_data, PlotType.DURATION_TPS, plot_id=id)
        self._plot(plot_lat_data, PlotType.DURATION_LATENCY, plot_id=id)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        prog='IOTA Plotter',
        description='Simple script to plot measurement data'
    )
    parser.add_argument(
        '--dir', default='./', help='Data directory'
    )
    parser.add_argument(
        '--shared-objects-ratio', nargs='+', type=int, default=[0, 100],
        help='The ratio of shared objects to plot (in separate graphs)'
    )
    parser.add_argument(
        '--committee', nargs='+', type=int, default=[4],
        help='The committee sizes to plot on the same graph'
    )
    parser.add_argument(
        '--faults', nargs='+', type=int, default=[0],
        help='The number of faults to plot on the same graph'
    )
    parser.add_argument(
        '--max-latencies', nargs='+', type=float, default=[2],
        help='The latency cap (in seconds) for scalability graphs'
    )
    parser.add_argument(
        '--y-max', type=float, default=None,
        help='The maximum value of the y-axis for L-graphs'
    )
    parser.add_argument(
        '--legend-columns', type=int, default=1,
        help='The number of columns of the legend'
    )
    parser.add_argument('--inspect', help='The measurement file to inspect')
    parser.add_argument(
        '--precision', type=float, default=30.0,
        help='The granularity of the duration when aggregating results'
    )
    args = parser.parse_args()

    if args.inspect is not None:
        parameters = PlotParameters(None, None, None)
        plotter = Plotter(
            args.dir, parameters, args.y_max, args.legend_columns, median=False
        )
        plotter.plot_inspect(args.inspect)
        plotter.plot_duration(args.inspect, args.precision)
    else:
        for r in args.shared_objects_ratio:
            parameters = PlotParameters(r, args.committee, args.faults)
            plotter = Plotter(
                args.dir, parameters, args.y_max, args.legend_columns, median=False
            )
            plotter.plot_latency_throughput()
            plotter.plot_health()
            plotter.plot_scalability(args.max_latencies)
