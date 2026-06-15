# Assets Required

This folder should contain the following media files for the website:

## Required Assets

### 1. `hero_image.jpg`
- **Purpose**: Background image for the home/hero section
- **Format**: JPG or PNG
- **Recommended specs**:
  - Resolution: 1920x1080 (Full HD) or higher
  - Aspect ratio: 16:9
  - File size: Under 1MB for optimal loading
- **Notes**: Should be visually appealing and work well with text overlay. A dark overlay is applied for text readability.

### 2. YouTube Video
- **Purpose**: Video showing the working model of the water-turret system
- **Format**: YouTube video link
- **Usage**: Used in Problem and Solution sections
- **Setup**: 
  1. Upload your video to YouTube
  2. Get the video ID from the YouTube URL (e.g., from `https://www.youtube.com/watch?v=VIDEO_ID`)
  3. Replace `'YOUR_VIDEO_ID_HERE'` in `src/components/Problem.tsx` and `src/components/Solution.tsx` with your actual video ID
  4. You can use the same video ID for both sections or different videos for each

### 3. `infrared_demo.jpg`
- **Purpose**: Technology demonstration showing infrared vision capabilities
- **Format**: JPG or PNG
- **Recommended specs**:
  - Resolution: 1200x800px or higher
  - File size: Under 500KB
- **Usage**: Used in Technology section

### 4. `team_placeholder.jpg` (or individual team photos)
- **Purpose**: Team member photos
- **Format**: JPG or PNG
- **Recommended specs**:
  - Resolution: 400x400px (square)
  - File size: Under 200KB per image
- **Usage**: Used in Team section
- **Note**: You can use one placeholder image for all team members, or create individual images for each team member and update the paths in `src/components/Team.tsx`

## Image Fallbacks

The website components include error handling that will display placeholder divs if images are not found. However, for the best user experience, please add the actual images.

## Getting Started

1. Add your hero image as `hero_image.jpg`
2. Add your product video as `working_model.mp4`
3. Add your technology demo image as `infrared_demo.jpg`
4. Add team photos (you can start with one `team_placeholder.jpg` or add individual photos)
5. Test the website to ensure all assets load correctly

## Free Stock Resources

If you need placeholder images while developing:
- **Videos**: [Pexels](https://www.pexels.com/videos/), [Pixabay](https://pixabay.com/videos/)
- **Images**: [Unsplash](https://unsplash.com/), [Pexels](https://www.pexels.com/), [Pixabay](https://pixabay.com/)

## Current Image Credits

- `hero_image.jpg` – image from the Insurance Institute for Business & Home Safety (IBHS), used to illustrate ember exposure on a structure.
- `wildfire_jam.png` – frame based on coverage from FOX 11 Los Angeles, “Palisades Fire: Abandoned cars,” https://www.foxla.com/news/palisades-fire-abandoned-cars.
