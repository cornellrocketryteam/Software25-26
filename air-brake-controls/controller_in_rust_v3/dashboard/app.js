const altSlider = document.getElementById('alt');
const velSlider = document.getElementById('vel');
const altVal = document.getElementById('alt-val');
const velVal = document.getElementById('vel-val');
const runBtn = document.getElementById('run-btn');

const actualApogeeEl = document.getElementById('actual-apogee');
const errorValEl = document.getElementById('error-val');

let chart;

altSlider.oninput = () => altVal.innerText = altSlider.value;
velSlider.oninput = () => velVal.innerText = velSlider.value;

function initChart() {
    const ctx = document.getElementById('telemetryChart').getContext('2d');
    chart = new Chart(ctx, {
        type: 'line',
        data: {
            labels: [],
            datasets: [
                {
                    label: 'Altitude (m)',
                    data: [],
                    borderColor: '#38bdf8',
                    yAxisID: 'y',
                    tension: 0.1
                },
                {
                    label: 'Predicted Apogee (m)',
                    data: [],
                    borderColor: '#f472b6',
                    borderDash: [5, 5],
                    yAxisID: 'y',
                    tension: 0.1
                },
                {
                    label: 'Velocity (m/s)',
                    data: [],
                    borderColor: '#fbbf24',
                    yAxisID: 'y1',
                    tension: 0.1
                },
                {
                    label: 'Deployment (%)',
                    data: [],
                    borderColor: '#10b981',
                    yAxisID: 'y2',
                    tension: 0.1,
                    fill: true,
                    backgroundColor: 'rgba(16, 185, 129, 0.1)'
                }
            ]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            interaction: { mode: 'index', intersect: false },
            plugins: {
                legend: { labels: { color: '#f8fafc' } }
            },
            scales: {
                x: { ticks: { color: '#94a3b8' }, grid: { color: 'rgba(255,255,255,0.1)' } },
                y: { 
                    type: 'linear', position: 'left', 
                    ticks: { color: '#38bdf8' }, 
                    grid: { color: 'rgba(255,255,255,0.1)' },
                    title: { display: true, text: 'Altitude (m)', color: '#38bdf8' }
                },
                y1: { 
                    type: 'linear', position: 'right', 
                    ticks: { color: '#fbbf24' }, 
                    grid: { drawOnChartArea: false },
                    title: { display: true, text: 'Velocity (m/s)', color: '#fbbf24' }
                },
                y2: { 
                    type: 'linear', position: 'right', 
                    min: 0, max: 100,
                    ticks: { color: '#10b981' }, 
                    grid: { drawOnChartArea: false },
                    title: { display: true, text: 'Deployment (%)', color: '#10b981' }
                }
            }
        }
    });
}

async function runSimulation() {
    runBtn.innerText = "Simulating...";
    runBtn.disabled = true;

    try {
        const res = await fetch(`/simulate?alt=${altSlider.value}&vel=${velSlider.value}`);
        const data = await res.json();

        if (data.error) {
            alert("Error: " + data.error);
            return;
        }

        // Update Chart
        chart.data.labels = data.map(d => d.time.toFixed(1));
        chart.data.datasets[0].data = data.map(d => d.alt);
        chart.data.datasets[1].data = data.map(d => d.pred_apogee);
        chart.data.datasets[2].data = data.map(d => d.vel);
        chart.data.datasets[3].data = data.map(d => d.deploy);
        chart.update();

        // Update Stats
        const finalAlt = data[data.length - 1].alt;
        actualApogeeEl.innerHTML = `${finalAlt.toFixed(1)} <span class="unit">m</span>`;
        
        const error = finalAlt - 3048.0;
        let errColor = Math.abs(error) < 50 ? '#10b981' : '#f43f5e';
        errorValEl.innerHTML = `<span style="color: ${errColor}">${error > 0 ? '+' : ''}${error.toFixed(1)}</span> <span class="unit">m</span>`;

    } catch (e) {
        alert("Error running simulation");
        console.error(e);
    } finally {
        runBtn.innerText = "Run Simulation";
        runBtn.disabled = false;
    }
}

runBtn.onclick = runSimulation;
initChart();
runSimulation(); // Initial run
