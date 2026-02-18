import pandas as pd
import numpy as np
import os

import matplotlib.pyplot as plt

# Get the directory where the script is located
script_dir = os.path.dirname(os.path.abspath(__file__))
data_dir = os.path.join(script_dir, 'data')

# Create the plot
fig, ax = plt.subplots(figsize=(12, 6))

# Read the CSV file
df = pd.read_csv("data/iota_def.csv")

# Convert the DataFrame to a NumPy array for easier manipulation
data = df.to_numpy()

bp1 = ax.boxplot(data, positions=np.arange(1, data.shape[1] + 1), widths=0.6, patch_artist=True,
              boxprops=dict(facecolor='lightblue', color='black'),
              medianprops=dict(color='blue'), flierprops=dict(marker='o', markeredgecolor='blue', markersize=5),
              capprops=dict(color='blue'), whiskerprops=dict(color='blue'))

# Read the CSV file
df = pd.read_csv("data/sui_def.csv")
data = df.to_numpy()

bp2 = ax.boxplot(data, positions=np.arange(1, data.shape[1] + 1), widths=0.6, patch_artist=True,
                boxprops=dict(facecolor='pink', color='black'),
                medianprops=dict(color='red'), flierprops=dict(marker='D', markeredgecolor='red', markersize=5),
                capprops=dict(color='red'), whiskerprops=dict(color='red'))

# Labels and title
ax.set_xlabel(r'$m$', fontsize=18)
ax.set_ylabel('Cancellation rate (%)', fontsize=18)
ax.grid(True, alpha=0.3)

ax.legend([bp2["boxes"][0],bp1["boxes"][0]], ['Baseline', 'IOTA'], loc='right', fontsize=18)

plt.tight_layout()
    
# Save the plot
output_file = os.path.join(script_dir, 'cancellation_plot.png')
plt.savefig(output_file, dpi=300, bbox_inches='tight')


# Create the plot
fig, ax = plt.subplots(figsize=(12, 6))

# Read the CSV file
df = pd.read_csv("data/iota_worker.csv")

# Convert the DataFrame to a NumPy array for easier manipulation
data = df.to_numpy()

bp1 = ax.plot(np.mean(data, axis=0), linestyle='None', marker='o', markerfacecolor='None', markeredgecolor='blue')

# Read the CSV file
df = pd.read_csv("data/sui_worker.csv")
data = df.to_numpy()

bp2 = ax.plot(np.mean(data, axis=0), linestyle='None', marker='D', markerfacecolor='None', markeredgecolor='red')

# Labels and title
ax.set_xlabel(r'$m$', fontsize=18)
ax.set_ylabel('Average active workers', fontsize=18)
ax.grid(True, alpha=0.3)
ax.legend(['IOTA', 'Baseline'], loc='right', fontsize=18)

plt.tight_layout()
    
# Save the plot
output_file = os.path.join(script_dir, 'avg_worker_plot.png')
plt.savefig(output_file, dpi=300, bbox_inches='tight')

# Create the plot
fig, ax = plt.subplots(figsize=(12, 6))

# Read the CSV file
df = pd.read_csv("data/iota_worker.csv")

# Convert the DataFrame to a NumPy array for easier manipulation
data = df.to_numpy()

bp1 = ax.boxplot(data, positions=np.arange(1, data.shape[1] + 1), widths=0.6, patch_artist=True,
              boxprops=dict(facecolor='lightblue', color='black'),
              medianprops=dict(color='blue'), flierprops=dict(marker='o', markeredgecolor='blue', markersize=5),
              capprops=dict(color='blue'), whiskerprops=dict(color='blue'))

# Read the CSV file
df = pd.read_csv("data/sui_worker.csv")
data = df.to_numpy()

bp2 = ax.boxplot(data, positions=np.arange(1, data.shape[1] + 1), widths=0.6, patch_artist=True,
                boxprops=dict(facecolor='pink', color='black'),
                medianprops=dict(color='red'), flierprops=dict(marker='D', markeredgecolor='red', markersize=5),
                capprops=dict(color='red'), whiskerprops=dict(color='red'))

# Labels and title
ax.set_xlabel(r'$m$', fontsize=18)
ax.set_ylabel('Number of concurrent workers', fontsize=18)
ax.grid(True, alpha=0.3)
ax.legend([bp2["boxes"][0], bp1["boxes"][0]], ['Baseline', 'IOTA'], loc='right', fontsize=18)

plt.tight_layout()
    
# Save the plot
output_file = os.path.join(script_dir, 'worker_plot.png')
plt.savefig(output_file, dpi=300, bbox_inches='tight')
