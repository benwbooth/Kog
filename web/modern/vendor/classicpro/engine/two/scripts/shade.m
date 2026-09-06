#include <lib/std.mi>
#include "../../lib/ClassicProFile.mi"
#include <lib/application.mi>

Function String getMyFile();

Global Group mainGroup, g_SecStop, g_SecShuf, g_SecRep, g_SecSeek, g_SecVis, g_SecVol, g_SecAot, g_SecOpen;

Global Container main;
Global Layout shade;
Global Layer overlay1;
Global Button b_ml_playlists, b_ml_playlists_fake, b_temp;
Global GuiObject gui_vis;

System.onScriptLoaded() {
	mainGroup = getScriptGroup();

	main = System.getContainer("main");
	shade = main.getLayout("shade");

	g_SecStop = mainGroup.getObject("shade.sect.stop");
	g_SecShuf = mainGroup.getObject("shade.sect.shuf");
	g_SecRep = mainGroup.getObject("shade.sect.rep");
	g_SecOpen = mainGroup.getObject("shade.sect.open");
	g_SecSeek = mainGroup.getObject("shade.sect.seek");
	g_SecAot = mainGroup.getObject("shade.sect.aot");
	g_SecVis = mainGroup.getObject("shade.sect.vis");
	g_SecVol = mainGroup.getObject("shade.sect.volume");
	b_ml_playlists = mainGroup.getObject("shade.mlmenu");
	b_ml_playlists_fake = mainGroup.getObject("shade.mlmenu.fake");
	
	gui_vis = mainGroup.findObject("shade.vis");
	
	overlay1 = g_SecVis.getObject("shade.vis.overlay");
	
	Map myMap = new Map;

	myMap.loadMap("playback.button.shuf.over.1");
	if(myMap.getHeight()==30){
		b_temp = mainGroup.findObject("overlay.shuf");
		b_temp.show();
		b_temp = mainGroup.findObject("overlay.rep");
		b_temp.show();
		b_temp=NULL;
	}



	// Reader for classic vis colors from bitmap (wa5.51)
	myMap.loadMap("shade.color.read");
	
	gui_vis.setXmlParam("colorbandpeak",integerToString(myMap.getARGBValue(0,0,2))+","+integerToString(myMap.getARGBValue(0,0,1))+","+integerToString(myMap.getARGBValue(0,0,0)));
	
	for(int i=2;i<18;i++){
		gui_vis.setXmlParam("colorband"+integerToString(18-i),integerToString(myMap.getARGBValue(0,i,2))+","+integerToString(myMap.getARGBValue(0,i,1))+","+integerToString(myMap.getARGBValue(0,i,0)));
	}

	for(int i=0;i<5;i++){
		gui_vis.setXmlParam("colorosc"+integerToString(5-i),integerToString(myMap.getARGBValue(2,i,2))+","+integerToString(myMap.getARGBValue(2,i,1))+","+integerToString(myMap.getARGBValue(2,i,0)));
	}
	delete myMap;
}

b_ml_playlists.onLeftClick(){
	b_ml_playlists_fake.leftClick();
}
b_ml_playlists.onRightButtonUp(int x, int y){
	complete;
	ClassicProFile.exploreFile(getMyFile());
}
b_ml_playlists.onLeftButtonDblClk (int x, int y){
	Application.Shutdown();
}
String getMyFile() {
//debug(getPlayItemMetaDataString("filename"));
	String bs = strleft("\ ",1);
	String output = "";

	if(System.strleft(System.getPlayItemString(),6) == "cda://") output = System.strmid(System.getPlayItemString(), 6, 1)+":"+bs;
	else output= getPlayItemMetaDataString("filename");

//debug(output);

	return output;
}

mainGroup.onResize(int x, int y, int w, int h){
	if(w>=240){
		g_SecStop.hide();
		g_SecShuf.hide();
		g_SecRep.hide();
		g_SecOpen.hide();
		g_SecSeek.hide();
		g_SecAot.hide();
		g_SecVis.hide();
		g_SecVol.hide();
	
	}

	if(w<254){						//0
		//nothing yet.. smallest size
	}
	else if(w<274){					//1
		g_SecStop.show();
	}
	else if(w<294){					//2
		g_SecStop.show();
		g_SecShuf.show();
	}
	else if(w<314){					//3
		g_SecStop.show();
		g_SecShuf.show();
		g_SecRep.show();
	}
	else if(w<346){					//4
		g_SecStop.show();
		g_SecShuf.show();
		g_SecRep.show();
		g_SecOpen.show();
	}
	else if(w<443){					//5
		g_SecStop.show();
		g_SecShuf.show();
		g_SecRep.show();
		g_SecOpen.show();
		g_SecSeek.show();
		
		g_SecSeek.setXmlParam("x", "234");
		g_SecSeek.setXmlParam("w", "-321");
	}
	else if(w<546){					//6
		g_SecStop.show();
		g_SecShuf.show();
		g_SecRep.show();
		g_SecOpen.show();
		g_SecSeek.show();
		g_SecAot.show();

		g_SecSeek.setXmlParam("x", "234");
		g_SecSeek.setXmlParam("w", "-346");
	}
	else if(w<701){					//7
		g_SecStop.show();
		g_SecShuf.show();
		g_SecRep.show();
		g_SecOpen.show();
		g_SecSeek.show();
		g_SecAot.show();
		g_SecVis.show();

		g_SecSeek.setXmlParam("x", "289");
		g_SecSeek.setXmlParam("w", "-401");
		g_SecVis.setXmlParam("x", "234");
	}
	else{							//8
		g_SecStop.show();
		g_SecShuf.show();
		g_SecRep.show();
		g_SecOpen.show();
		g_SecSeek.show();
		g_SecAot.show();
		g_SecVis.show();
		g_SecVol.show();


		g_SecSeek.setXmlParam("x", "396");
		g_SecSeek.setXmlParam("w", "-508");
		g_SecVis.setXmlParam("x", "341");
	}
}